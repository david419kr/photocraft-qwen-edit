# waifu2x-swin_unet_v3 / art 추가 분석·구현 계획

최신 업데이트: [TensorRT GPU 전후처리 구현·검증](waifu2x-gpu-prepost-validation.md). 기본 추론의 전후처리는 GPU로 변경했으며 기존 CPU 결과와 동일함을 확인했다.

작성일: 2026-11-10  
상태: 실제 제공 이미지의 대표 8건 TensorRT 비교를 통과해 **TensorRT 경로로 확정**. 앱 통합은 미구현. [최신 검증 결과](waifu2x-tensorrt-validation.md).  
상위 계획: [외부 백엔드 업스케일](external-backends-upscale.md).

## 1. 확정 요구

업스케일 모델 선택에 **`waifu2x-swin_unet_v3`** 한 항목을 추가한다.
대상은 2026년 공개한 공식 **`swin_unet_v3/art`**이며, 구형 `swin_unet/art`,
`cunet`, photo, art_scan 가중치를 추가하지 않는다.

| 설정 | 선택값 | 기본값 |
|---|---|---|
| 배율 | 선택 UI 없음. 이 항목은 네이티브 2배 고정 | 2배 |
| 노이즈 제거 | None / Low / Medium / High / Highest | Low |
| 타일 크기 | 64 / 256 / 400 / 640 | 640 |
| 알파 채널 | Auto / Disable | Auto |
| TTA | ON / OFF | ON |
| 배치 크기 | 1 / 4 / 8 / 16 / 32 | 4 |
| GPU | 기존 공통 GPU 선택 사용 | 자동 |

위 기본값은 사용자 지정이다. 공식 CLI의 기본 타일 256, 기본 TTA OFF와 혼동하지 않는다.
모델을 바꿔도 각 모델의 설정을 별도로 기억한다. IllustrationJaNai의 512/128 설정을
waifu2x에 자동 적용하지 않는다.

타일링 OFF는 공식 GUI/CLI에서 제공하지 않는다. 초기 waifu2x 통합에서는 공식 타일
경로만 쓰는 안을 권장했다. 별도 OFF가 필요하면 V3의 full-frame 패딩·잘림·알파·TTA를
검증하는 추가 범위로 다룬다. IllustrationJaNai에 확정한 ON/OFF는 그대로 유지한다.

## 2. 공식 구현과 버전 확인

- 개발 저장소: [nagadomi/nunif](https://github.com/nagadomi/nunif).
- 고정 소스: `dev`, 커밋 `b97423c786dfb998dc39d1f0ef1d688ada89c57e`.
- 공식 발표: unlimited:waifu2x의 2026-08-16 `swin_unet_v3.art` 추가 기록.
- 가중치: 공식 release tag `0.0.0`의
  `waifu2x_pretrained_models_swin_unet_v3_art_20260816.zip`.
- 파일 크기: **115,825,363 bytes**.
- GitHub 자산 digest와 다운로드한 ZIP의 SHA-256 일치:
  `d623590525e4438b92d8cc0404ff58c427ab79b79c4b02de12ce132e6d89e82e`.

작성자는 V3가 dev branch에 있다고 안내한다. 일반 README의 `art` 또는
`torch.hub.load(...:master, model_type="art")`를 그대로 쓰면 요청한 V3가 아닌 이전 모델을
선택할 수 있다. 다운로드 함수의 기본 전체 모델 묶음도 실행하지 않는다.
**고정한 로컬 소스 + 명시적인 V3 모델 디렉터리**를 이용하고 checkpoint 내부 이름까지 확인한다.

출처: [공식 웹 앱](https://unlimited.waifu2x.net/),
[작성자 답변](https://github.com/nagadomi/nunif/discussions/730),
[공식 릴리즈](https://github.com/nagadomi/nunif/releases/tag/0.0.0),
[고정 소스의 GUI 모델 등록](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/waifu2x/gui.py).

## 3. 준비한 자료와 실행 환경

실행 파일 옆의 다음 폴더에 준비했다. 아직 실행 가능한 PhotoCraft `backend.json`은 만들지 않았다.

```text
external-backends/waifu2x-swin_unet_v3/
  waifu2x_pretrained_models_swin_unet_v3_art_20260816.zip
  nunif-<commit>-source.zip
  source/nunif-<commit>/             # 고정 소스 및 LICENSE
  models-source/pretrained_models/   # 공식 ZIP의 원형
  models/art/                       # 통합에 사용할 2배 모델 5개
  python/                           # 독립 CPython과 CUDA 패키지; venv 아님
  preparation.json
  model-inventory.json
  requirements-prepared.lock.txt
  gpu-probe.json
```

공식 ZIP에는 V3의 1배 노이즈 제거 모델도 들어 있지만, 통합용 `models/art`에는 필요한
**2배 가중치 5개만** 배치했다. 구 아키텍처의 가중치는 다운로드하지 않았다.
소스에 다른 아키텍처의 등록 코드가 있다는 사실과 그 모델 가중치를 설치하는 것은 구분한다.

현재 전용 환경:

| 구성 | 확인 값 |
|---|---|
| Python | CPython 3.12.13 Windows x64, 자체 표준 라이브러리·DLL·include/libs 포함 |
| PyTorch | `2.14.1+cu132` |
| torchvision | `0.29.1+cu132` |
| CUDA 계열 | PyTorch CUDA 13.2 |
| 추가 직접 의존성 | NumPy, Pillow, packaging, tqdm, requests |
| 재현 정보 | 설치 패키지 전체 버전은 `requirements-prepared.lock.txt` |
| GPU | RTX 4090, compute capability 8.9 |
| 시스템 설치 영향 | 기존 Python/ComfyUI에 패키지를 설치하거나 수정하지 않음 |

Torch/torchvision 버전은 고정 소스의 Windows CUDA 요구와 맞췄다.
`triton-windows`는 `torch.compile`을 사용할 때의 추가 준비물이며, 현재 eager 추론에
필요하지 않아 설치하지 않았다. 전체 nunif requirements의 iw3·웹 서버·GUI 의존성도 설치하지 않았다.
[공식 CUDA 요구](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/requirements-torch.txt),
[공식 Python API](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/waifu2x/docs/python_api.md).

준비물의 hash·checkpoint 메타데이터·실측값은
[준비 및 검사 JSON](waifu2x-swin-unet-v3-preparation.json)에 보관한다.

## 4. 노이즈 옵션은 가중치 선택이다

| UI 선택 | 공식 호출 | 실제 파일 |
|---|---|---|
| None | `method="scale"`, noise `-1` | `scale2x.pth` |
| Low | `method="noise_scale"`, noise `0` | `noise0_scale2x.pth` |
| Medium | 동일 method, noise `1` | `noise1_scale2x.pth` |
| High | 동일 method, noise `2` | `noise2_scale2x.pth` |
| Highest | 동일 method, noise `3` | `noise3_scale2x.pth` |

노이즈 강도를 연속 숫자로 바꾸거나 1배 denoise 후 2배 모델을 추가 실행하는 것이 아니다.
노이즈 제거와 확대를 함께 학습한 해당 2배 모델을 선택한다.
5개 모두 실제 로드 결과가 `waifu2x.swin_unet_v3_2x`이고, 가중치는 FP32다.
모델 kwargs는 RGB 3채널, encoder 64, middle 128, decoder 96으로 확인했다.

알파 Auto에서 노이즈를 선택해도 알파 처리에는 노이즈 없는 `scale2x.pth`가 필요하다.
따라서 Low 파일 하나만 배포하면 안 된다. 현재 5개 합계는 72,616,539 bytes다.
각 파일의 SHA-256은 검사 JSON에 포함했다.

## 5. 타일 400의 공식 동작과 겹침

고정한 V3 소스는 `size >= 64`이고 **32의 배수**인 타일을 허용한다.
공식 `find_valid_tile_size`는 입력한 값에서 내려가며 유효한 크기를 찾는다.

| UI에서 요청 | 실제 모델 입력 | 입력 타일 이동 간격 |
|---:|---:|---:|
| 64 | 64 | 44 |
| 256 | 256 | 236 |
| 400 | **384** | 364 |
| 640 | 640 | 620 |

UI 선택 목록은 사용자 요청대로 유지하되, 400을 선택하면 공식 보정에 따른 실제 크기
384를 툴팁이나 유효 크기 표시로 알린다. 400을 그대로 네트워크에 넣거나, 공식과 달리
416으로 올리지 않는다.

이 모델은 `i2i_scale=2`, 출력 기준 `offset=16`, `blend_size=8`을 사용한다.
공식 SeamBlending의 계산에서 입력 문맥은 한쪽 8px, 입력 기준 blend는 4px이고,
이동 간격은 `tile - (8*2 + 4)`가 된다. 네트워크의 타일 출력에는 경계 잘림이 있기 때문에
모델에 타일을 넣은 결과 자체와 전체 이미지의 최종 2배 치수도 구분해야 한다.

IllustrationJaNai/libaji의 512/128 겹침 알고리즘을 여기에 가져오지 않는다.
waifu2x는 공식 패딩·잘림·SeamBlending을 사용하고, 별도 겹침 조절값을 노출하지 않는 안이다.
이는 임의로 고른 작은 겹침이 아니라 해당 V3 구현의 경계 처리다.

출처:
[V3 크기·offset 정의](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/waifu2x/models/swin_unet_v3.py),
[유효 타일 보정](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/nunif/models/model.py),
[SeamBlending](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/nunif/utils/seam_blending.py).

## 6. TTA·배치·알파 의미

### TTA

공식 TTA는 원본·반전·회전 조합 **8개**의 RGB 추론 결과를 방향 복원 후 평균한다.
TTA가 더 큰 배율을 만들거나 랜덤한 결과를 선택하는 기능은 아니다.
추론량이 늘지만 모든 이미지에서 화질 개선이 보장되는 것은 아니므로 ON/OFF를 보존한다.
아트 V3는 공식 GUI에서 TTA 대상에 포함된다.
[TTA 정의](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/nunif/transforms/tta.py).

### 배치

배치 크기는 **동시에 네트워크에 넣는 타일 수**다. 이미지 파일 수나 노이즈 모델 수가 아니다.
현재 공식 변환 코드는 TTA의 8개 방향을 순차 처리하고, 각 방향 내부에서 타일을 배치화한다.
큰 배치가 항상 빠른 것도 아니고, 배치 32가 모든 GPU/이미지에서 들어가는 것도 아니다.
메모리 부족이면 타일/배치를 낮추도록 오류를 보여 주고 설정을 몰래 바꾸지 않는다.
마지막 불완전 배치도 지원한다.

### Alpha Auto / Disable

- **Auto:** 알파가 없거나 전부 불투명하면 불필요한 추가 처리를 피한다. 실제 투명도가 있으면
  공식 AlphaBorderPadding을 사용하고, 노이즈 없는 `scale2x.pth`로 알파를 별도 확대한다.
  공식 경로에서 RGB TTA와 알파 처리는 같지 않다. 알파에 임의로 8방향 TTA를 추가하지 않는다.
- **Disable:** 공식 웹 구현처럼 RGB를 흰 배경에 합성하고 투명도를 제거하는 안이다.
  원본 알파를 무시한 검정 테두리나, 숨겨진 투명 RGB가 그대로 노출되는 결과를 피한다.
  UI에 ‘투명도 제거·흰 배경’이라는 결과를 알려 준다.

기존 IllustrationJaNai 계획의 ‘원본 알파를 일반 보간으로 확대’를 waifu2x 결과 위에
다시 적용하면 공식 Auto 결과를 덮어쓰게 된다. 공통 외부 계약에 입력/출력 알파 파일을
선택적으로 포함하고, 모델별 알파 정책을 구분해야 한다.
Disable 결과에는 호스트가 원본 알파를 복원하지 않는다.

출처:
[공식 변환 경로](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/waifu2x/utils.py),
[공식 웹 입력 처리](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/waifu2x/unlimited_waifu2x/public_html/js/runner.js).

## 7. NVIDIA 실행 방식 결정

**최종 선택은 TensorRT 혼합 FP16/FP32 경로다.** 아래의 PyTorch 기준 경로는 최초 준비·비교 단계의 기록이며, 배포 권장안은 [실제 검증 결과](waifu2x-tensorrt-validation.md)로 갱신했다. 공식 PyTorch CUDA + FP16 AMP는 변환·검증 기준으로 유지한다.

최초 준비 단계에서는 공식 PyTorch CUDA + FP16 AMP를 사용했다.
Nunif의 변환·패딩·TTA·알파를 직접 사용하고, Pillow 파일 입출력 대신 FP32 tensor
입출력을 연결한다. 이는 기존 16/32-bit 문서를 8-bit PNG로 낮추지 않기 위해서다.
색 프로파일 변환과 문서 확대/Undo는 기존 Rust 계획을 사용한다.

| 경로 | 현재 확인 | 판단 |
|---|---|---|
| 공식 PyTorch CUDA + AMP | 준비한 환경에서 실제 5모델·TTA·알파 추론 성공 | 초기 기준 경로 |
| PyTorch FP32 | 실제 추론 및 단일 합성 입력 비교 성공 | 정밀도 검증용 고급 옵션 후보 |
| torch.compile + Triton Windows | 공식 지원 안내 있음. 현재 설치·벤치하지 않음 | cold/warm 시간과 배포 조건을 검증한 뒤 선택 |
| ONNX + TensorRT | 5모델 변환·엔진 생성 및 실제 이미지 대표 8건 비교 통과 | **배포 경로로 확정**. 현재 검증용 CPU 전후처리는 PyTorch보다 느림 |
| 구형 ONNX/Vulkan waifu2x | 요청한 V3/art 가중치와 같은 구현으로 볼 수 없음 | 이번에 채택하지 않음 |

IllustrationJaNai의 BF16 TensorRT 전제는 유지한다. 새 waifu2x까지 이름만 맞춰 BF16으로
강제하거나 기존 구형 ONNX 묶음을 가져오지 않는다.
waifu2x의 공식 AMP 기본은 CUDA FP16이다. PTH 가중치 FP32와 실행 AMP 타입도 구분한다.

초기 계획에서는 TensorRT를 후속 검증 대상으로 두었다. 이후 대표 검증을 통과해 배포 경로로 확정했다. 구현 시에도 새 V3의 5파일을 각각 export하고 parser·offset·
타일·가변 배치·TTA·알파의 동등성을 검사한 뒤 전체 클릭 지연과 VRAM을 비교한다.
순수 네트워크 시간만 줄고 프로세스 기동/엔진 생성 비용이 커지는 안을 ‘최적’이라고 판단하지 않는다.
실제 비교 결과와 제한은 최신 검증 문서에 기록했다.

### 추가로 제안할 고급 옵션

1. **배포 정밀도:** 검증된 TensorRT 혼합 정밀도를 기본으로 한다. PyTorch AMP/FP32는 개발 비교용이다. 검증하지 않은 BF16·INT8을 추가하지 않는다.
2. **torch.compile:** 배포를 TensorRT로 정했으므로 사용자 옵션에서 제외한다. 개발 비교 필요 시에만 검토한다.
3. **엔진 캐시 상태/재생성 및 workspace:** TensorRT 경로에 맞춰 제공한다. 모델·타일·배치 프로파일을 캐시에서 구분한다.

노이즈 수치를 연속 슬라이더로 만들거나 CFG/steps/seed를 추가하지 않는다.
GPU 선택은 공통 설정을 사용한다. 우선 단일 CUDA GPU로 구현하며 멀티 GPU·다중 이미지 일괄 처리는 제외한다.

출처: [Python API의 AMP](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/waifu2x/docs/python_api.md),
[V3 ONNX export 함수](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/waifu2x/export_onnx.py),
[Windows compile 준비물](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/windows_package/docs/torch_compile.md).

## 8. 이번 GPU 측정의 정확한 범위

조건: RTX 4090, CUDA 13.2, Torch 2.14.1, eager 실행, Low, 타일 640, 배치 4.
960×640의 단일 합성 RGB 입력을 1920×1280으로 확대했다. 한 번 워밍업하고 한 번 시간을 쟀다.
각 측정 전후 CUDA synchronize를 호출했다.

| 실행 | 시간 | PyTorch peak allocated |
|---|---:|---:|
| FP16 AMP, TTA OFF | 약 0.262초 | 약 3,601 MiB |
| FP32, TTA OFF | 약 0.342초 | 약 5,885 MiB |
| FP16 AMP, TTA ON | 약 2.049초 | 약 3,847 MiB |

- FP16 대 FP32 PSNR은 이 입력에서 약 81.5 dB였다. 복원 화질 평가나 모든 이미지의 오차 보증이 아니다.
- peak allocated는 **PyTorch 할당기의 최고 사용량**이다. CUDA context·다른 앱 등을 포함한 총 VRAM 수치가 아니다.
- 프로세스 시작·Torch import·모델 로드가 포함된 앱 클릭 시간도 아니다. 첫 두 모델 로드만은 별도로 약 0.30초였다.
- 테스트 중 5개 모델을 확인해 같은 context에 남아 있었다. 최종 워커의 메모리 측정은 새 프로세스와 실제 모델 로딩 정책으로 다시 한다.
- 작은 홀수 입력에서 노이즈 5단계 모두 최종 크기 정확히 2배, NaN/Inf 없음, Auto 알파의 크기·범위도 확인했다.
- 모든 타일/배치 조합, 실제 일러스트 화질, compile, TensorRT, RTX 30 시리즈 실기는 아직 검증하지 않았다.

이 측정은 첫 실행 경로 선택의 근거다. ‘가장 빠른 구현’ 또는 ‘다른 GPU에서도 같은 성능’의 증거로 사용하지 않는다.

## 9. 공통 계획에 필요한 변경

1. 모델 catalog 항목마다 네이티브 배율을 가진다. IllustrationJaNai는 4, 새 waifu2x는 2다.
   공통 `image.aiUpscale`과 문서 preflight는 선택한 모델 배율로 목표 치수를 계산한다.
2. 사용자에게 배율 선택을 보여 주지 않는다. 모델 선택에 따른 결과 치수만 표시한다.
3. 모델별 설정을 저장한다. waifu2x로 바꿨다고 다른 모델의 타일·겹침을 덮어쓰지 않는다.
4. waifu2x 선택 시 노이즈/타일/알파/TTA/배치만 보여 주고, IllustrationJaNai의 겹침·TRT workspace는 숨긴다.
5. 공통 RGB FP32 계약에 선택적 alpha FP32 입력·출력을 추가한다. 알파를 어느 쪽이 처리하는지 descriptor로 구분한다.
6. 모델 선택 라벨과 설정 capability는 외부 manifest/catalog가 제공한다. Rust 앱에 V3 모델 파일명이나 특정 공급자 경로를 고정하지 않는다.
7. 이 모델도 전체 현재 문서를 확대하고 한 번의 Undo/Redo로 복원한다. 기존 레이어 보존·표시와 좌표 처리 방식도 상위 계획과 같다.
8. 공식 소스는 고정한 로컬 파일만 import한다. 실행 시 GitHub에 연결하는 torch.hub 자동 다운로드나 master 업데이트를 사용하지 않는다.
9. ComfyUI와 독립된 외부 프로세스로 실행한다. 취소 시 프로세스를 종료하고 문서·Redo를 보존한다.

공통 문서의 배율 4배 및 알파 보간 관련 설명은 IllustrationJaNai 경로에 대한 것이다.
waifu2x는 이 문서의 2배 및 공식 Auto/Disable 정책을 적용한다.

## 10. 구현 전 추가 검증과 패키징

- 기본 640/4/TTA ON 조합을 여러 실제 일러스트로 측정하고, 노이즈 5단계를 비교한다.
- 64/256/400/640 선택과 400→384 보정이 공식과 같은지 확인한다.
- 배치 1/4/8/16/32는 실제 충분한 타일이 있는 입력으로 검사한다. OOM에서 자동 변경하지 않는다.
- Auto의 투명 경계, 전체 투명, 전체 불투명, Disable의 흰 배경 합성을 검증한다.
- FP32 tensor I/O, ICC, RGB/Gray/CMYK/Lab, 8/16/32-bit SDR을 공통 host 경로와 함께 검사한다.
- compile을 추가한다면 정확한 Triton Windows 버전·Python dev 파일·필요 도구를 함께 고정한다.
  warm 이득만 보지 않고 최초 시간, shape/배치 변경, 캐시 무효화, 실패 표시도 검사한다.
- runtime은 독립 Python과 실제 파일로 묶고 `.pth`·모델 경로를 패키지 상대 위치로 정한다.
  정션, 시스템 Python, 기존 ComfyUI, 개인 PC PATH에 의존하지 않는다.
- TensorRT 엔진이나 Torch compile 캐시는 환경 종속 생성물이다. 별도 PC에서 무조건 재사용하지 않는다.
- 다른 경로에서 실제 GPU 추론을 하고 `sys.prefix/base_prefix/path`, 모듈과 DLL 로딩 위치를 확인한다.
  같은 PC 이동 검증과 별도 PC 검증은 구분한다. VC 런타임·드라이버 조건도 배포 전에 점검한다.
- GPU 드라이버와 OS 외에 필요한 DLL을 확정하고 라이선스를 함께 보관한다.

공식 소스 LICENSE는 MIT다. 가중치 원본 ZIP에는 별도 LICENSE 파일이 없으므로 공개 배포 시
해당 공식 릴리즈의 조건과 출처를 재확인하고 기록한다. 현재 자료는 로컬 준비용이며 모델·CUDA
런타임을 Git이나 공개 릴리즈에 올리지 않았다.
[공식 LICENSE](https://github.com/nagadomi/nunif/blob/b97423c786dfb998dc39d1f0ef1d688ada89c57e/LICENSE).
