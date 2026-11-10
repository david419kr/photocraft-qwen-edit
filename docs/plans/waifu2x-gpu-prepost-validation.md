# waifu2x TensorRT GPU 전후처리 변경·검증

작성일: 2026-11-10

## 변경 결과

기존 NumPy CPU 경로를 검증용으로 분리하고 기본 `trt_runner.convert` 호출을 CUDA GPU 경로로 연결했다.
타일 준비·패딩·타일 합성·TTA 변환과 평균·알파 경계 확장·알파 확대·흰 배경 합성을 GPU에서 수행한다.
TensorRT 입력/출력은 GPU 포인터로 직접 연결한다. 타일마다 CPU로 왕복하지 않는다.
기존 TensorRT 모델·정밀도·엔진 파일은 변경하지 않았다. 앱 UI 통합은 아직 하지 않았다.

## 같은 조건의 처리 시간

RTX 4090과 기존 제공 이미지(1536×1239)를 사용했다. 각 경로별 1회 워밍업 후 3회 실행의 중앙값이다.
CPU FP32 입력 버퍼에서 CPU FP32 결과 버퍼까지 측정하며 이미지 디코드·PNG 저장·모델/엔진 로드·CUDA 컴파일은 제외했다.

| 사례 | 공식 PyTorch GPU | 이전 TensorRT + CPU NumPy | 새 TensorRT + GPU |
|---|---:|---:|---:|
| noise_low | 3.067s | 5.274s | 2.246s |
| alternate_tile256_batch8 | 0.413s | 0.620s | 0.332s |
| soft_alpha_auto | 0.799s | 3.602s | 0.585s |
| soft_alpha_disable | 0.409s | 0.601s | 0.301s |

기본 Low/타일640/배치4/TTA ON은 PyTorch 대비 약 26.8% 시간이 짧았고, 이전 NumPy 경로 대비 약 2.35배 빨랐다.
이 수치는 해당 이미지·GPU·설정의 결과다. 모든 이미지나 다른 GPU에서 같은 비율을 보장하지 않는다.

## 결과와 전송 확인

- 비교한 4건의 출력 PNG 픽셀은 이전 TensorRT+NumPy 결과와 모두 동일했다.
- 기본 설정의 FP32 RGB 결과 배열도 기존 결과와 완전히 일치했다.
- PyTorch 대비 기존 정밀도 비교 수치가 유지됐으며, 알파 Auto/Disable도 유지됐다.
- 기본 설정은 입력 RGB/알파를 각각 1회 올리고, 최종 RGB/알파를 각각 1회 내린다.
- 불투명 여부·오류 플래그 같은 4바이트 제어값을 가져오는 동기화는 있다. CPU 픽셀 처리나 타일 왕복이 아니다.
- 모든 비교 사례의 타일별 host copy 수는 0이었다.
- 원본 이미지 SHA-256은 변경되지 않았다.

## 코드 및 런타임

- `tools/prepost.cu`: CUDA 픽셀 커널.
- `tools/trt_gpu.py`: GPU 버퍼·커널·TensorRT 실행과 최종 출력 관리.
- `tools/trt_runner.py`: 기본 GPU 호출과 기존 NumPy 비교 함수 `convert_numpy_reference`.
- TensorRT context의 입력 shape를 바꿀 때 이전 실행과 경합하지 않도록 동기화한다.
- NVRTC 13.4.92는 공통 private Python 환경에 설치했다. 전체 CUDA Toolkit이나 시스템 Python을 실행 조건으로 삼지 않는다.
- CUDA 소스와 NVRTC 버전으로 만든 키의 PTX 캐시를 백엔드 안에 둔다. 경로는 파일 위치에서 계산한다.
- 기존 Python/PATH를 배제한 프로세스에서도 CUDA 커널 모듈을 로드했고, NVRTC DLL이 private runtime 안에서 로드되는 것을 확인했다.
- 해당 TensorRT 런타임에는 PyTorch가 설치돼 있지 않다. CuPy도 추가하지 않았다.
- NumPy는 CPU 입출력 버퍼와 스칼라 인자 전달에 남아 있다. 이미지 변환·타일 합성·TTA 계산에는 사용하지 않는다.

## 범위

사용자 요청대로 대표 4건(기본, 다른 타일/배치, 알파 Auto, 알파 Disable)을 확인했다. 전체 옵션 조합을 다시 실행하지 않았다.
다른 PC 패키지 배포, 다른 GPU 검증, PhotoCraft 버튼·설정·문서 Undo 통합은 이번 변경 범위에 포함하지 않았다.
[상세 측정 JSON](waifu2x-gpu-prepost-validation.json).
