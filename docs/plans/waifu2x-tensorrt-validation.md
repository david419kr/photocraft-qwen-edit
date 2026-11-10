# waifu2x V3 TensorRT 대표 검증 결과

작성일: 2026-11-10

이 문서는 초기 CPU NumPy 검증의 기록이다. 현재 기본 호출은 GPU 전후처리이며, 최신 속도와 결과는 [GPU 전후처리 검증](waifu2x-gpu-prepost-validation.md)을 따른다.

## 결정

**waifu2x-swin_unet_v3의 배포 추론 경로를 TensorRT로 확정한다.**
사용자 요청에 따라 과도한 조합 검증 대신 실제 제공 이미지의 대표 8건을 확인했다.
앱 UI/문서 명령 통합은 아직 수행하지 않았다. PyTorch 환경은 변환·기준 비교용으로 보관하며, 배포 추론에는 필요하지 않다.

## 환경과 변환

- RTX 4090, 드라이버 616.56.
- TensorRT 11.4.0.106, CUDA Python 13.5.0. CUDA runtime 질의 값은 13040.
- 공통 런타임: `external-backends/_runtimes/tensorrt-python312`. 해당 환경에는 PyTorch가 없다.
- 공식 V3 2배 가중치 5개를 공식 dynamo 경로로 FP32 ONNX 변환했다.
- 배포 후보는 FP32 입출력, 주 연산 FP16, 수치에 민감한 Pow/ReduceMean/Sqrt/Reciprocal/Softmax/ReduceSum/Div 등의 FP32 유지 조합이다.
- 이는 한 TensorRT 엔진 안의 혼합 정밀도다. PyTorch를 호출하거나 FP32 엔진과 FP16 엔진을 번갈아 실행하는 방식이 아니다.
- builder optimization level 1, workspace 상한 4 GiB. 최고 속도를 튜닝한 설정은 아니다.
- 원본 PTH/FP32 ONNX는 덮어쓰지 않고 변환 파일을 `onnx/mixed`에 별도 저장했다.

## 실제 이미지와 비교 방법

- 입력: 사용자 제공 PNG, 1536×1239 RGBA. 실제 알파는 전부 255였다.
- 모든 결과: 3072×2478. 원본 파일 SHA-256은 테스트 전후 동일했다.
- 기준: 고정 nunif V3 소스의 CUDA PyTorch AMP, 동일 옵션.
- 알파 검증은 같은 RGB에 별도의 부드러운 마스크를 적용한 파생 입력을 사용했다. 원본 수정은 하지 않았다.
- 유한값·출력 치수와 기준 대비 RGB/알파 수치를 확인했다. 여기서 PSNR은 원본 대비 복원 성능이 아니라 두 구현의 일치도를 뜻한다.
- 대표 비교의 기준으로 PSNR 60 dB 이상을 사용했다. 모델 제작자가 보증한 기준이나 모든 이미지의 화질 보증은 아니다.

## 결과

| 케이스 | 옵션 | RGB PSNR | TensorRT 시간 | PyTorch 시간 |
|---|---|---:|---:|---:|
| noise_none | noise=-1, tile=640, batch=4, TTA=OFF | 73.37 dB | 0.59s | 0.86s |
| noise_low | noise=0, tile=640, batch=4, TTA=ON | 78.04 dB | 5.54s | 3.09s |
| noise_medium | noise=1, tile=640, batch=1, TTA=OFF | 73.38 dB | 0.77s | 0.36s |
| noise_high | noise=2, tile=640, batch=4, TTA=OFF | 72.97 dB | 0.76s | 0.42s |
| noise_highest | noise=3, tile=640, batch=4, TTA=OFF | 72.98 dB | 0.75s | 0.42s |
| alternate_tile256_batch8 | noise=0, tile=256, batch=8, TTA=OFF | 73.69 dB | 0.64s | 0.43s |
| soft_alpha_auto | noise=0, tile=640, batch=4, TTA=OFF | 75.05 dB | 3.94s | 0.91s |
| soft_alpha_disable | noise=0, tile=640, batch=4, TTA=OFF | 74.75 dB | 0.77s | 0.40s |

**8건 모두 통과했다.** 기본 Low/640/4/TTA ON의 평균 차이는 8비트 기준 약 0.024단계, 최대 약 0.54단계였다.
노이즈 옵션은 None/Low/Medium/High/Highest 모두 실제 추론했다. 타일 256/배치 8, 배치 1, TTA ON/OFF 및 파생 입력의 알파 Auto/Disable을 포함했다.

## 속도 해석

현재 검증용 전후처리는 CPU NumPy다. 기본 TTA ON에서 TensorRT 약 5.54초, PyTorch 약 3.09초였으며, 현재 프로토타입이 더 빠르다는 근거는 없다.
시간은 단일 실행 관측값이고, 프로세스 생성/모델 로드/PNG 저장을 제외한 변환 구간이다. 엄밀한 벤치마크가 아니다.
TensorRT 선택은 출력 동등성·기능 동작·공통 런타임 사용 및 배포 시 PyTorch 제외 가능성을 근거로 한다.
통합 구현 시 GPU 전후처리와 복사 감소를 우선 검토한다. 이를 이미 완료한 최적화로 표현하지 않는다.

## 확인된 제약

- 모든 타일 크기와 batch 32, 동적 공간 크기를 하나의 넓은 프로파일에 넣은 빌드는 실패했다. 타일을 고정하고 테스트에 필요한 배치 범위를 사용한 엔진은 빌드·추론에 성공했다.
- 이번 검증은 타일 640에서 최대 6, 타일 256에서 최대 8인 프로파일 등을 사용했다. 배치 16/32, 타일 64/400과 전체 조합은 실행하지 않았다.
- 엔진 캐시는 모델·정밀도·TensorRT/GPU·고정 타일·지원 배치 범위를 구분해야 한다. 지원 범위를 넘는 요청을 조용히 축소하지 않는다.
- 공식 AMP 그래프를 CUDA에서 직접 export하는 별도 시도는 `aten._fused_rms_norm` export 미지원으로 실패했다. 검증에 사용한 것은 공식 FP32 export 후 명시적 혼합 정밀도 변환 경로다.
- 원본 모델을 그대로 전부 FP16으로 강제한 파일은 최종 선택하지 않았다. 정규화 관련 FP32 연산을 유지한 후보를 실제 출력과 비교했다.
- 다른 GPU/다른 PC 및 최종 설치 패키지는 검증하지 않았다.
- Janai 모델을 이 공통 TensorRT에서 추론하는 것은 이번에 실행하지 않았다.
- 기존 PyTorch 개발 폴더는 삭제하지 않았다. 배포에서 제외할 수 있다는 것과 실제 디스크 공간을 회수했다는 것은 구분한다.

## 파일 위치

- 결과 PNG와 상세 비교 JSON: 앱 폴더의 `upscale-tests/waifu2x-tensorrt-20261110/`.
- 대표 결과: `trt_noise_low.png`.
- 변환 모델: `external-backends/waifu2x-swin_unet_v3/onnx/mixed/`.
- 검증용 엔진: 해당 백엔드의 `cache/trt-test/`.
- PyTorch 없는 검증 러너: 해당 백엔드의 `tools/trt_runner.py`.
- 세부 수치와 ONNX 해시: [검증 JSON](waifu2x-tensorrt-validation.json).

## 계획 반영

IllustrationJaNai와 waifu2x는 같은 TensorRT/CUDA/Python 런타임을 사용하도록 구현한다. 각 모델의 가중치·엔진·설정은 별개다.
PyTorch·onnxscript·변환 도구·nunif 소스는 개발/비교용 자료이며 최종 추론 패키지에 필수로 넣지 않는다.
다음 단계는 앱의 외부 백엔드 계약, 설정 UI, 문서 확대·단일 Undo를 연결하는 작업이다.
