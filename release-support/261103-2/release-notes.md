PhotoCraft Local Qwen AI **261103-2-0.20.1** — upstream PhotoCraft **0.20.1** 기반 수정 릴리스입니다.

최초 `261103-0.20.1` 빌드는 CMYK 조정 레이어의 Fill 처리 오류를 확인한 뒤 공개를 중단했습니다. 이전 릴리스는 draft로 보존했으며 태그와 파일을 덮어쓰지 않았습니다. 이번 revision 2는 기존 0.20.1 설치보다 새 버전으로 인식됩니다.

### 수정 내용

- CMYK 조정 레이어에서 8개 특수 블렌드 모드의 Fill을 블렌드 내부에 적용해 exact/split 합성 경로를 일치시켰습니다.
- 클리핑된 RGB 조정 레이어와 GPU 계획에도 같은 규칙을 적용했습니다. 마스크·Opacity·알파 및 Dissolve 동작을 유지합니다.
- 최소 실패 입력을 8/16/32비트 결정적 테스트로 추가하고, 마스크·불투명도·클리핑 조합 및 GPU 계획 회귀 테스트를 추가했습니다.
- 이동된 테스트 모듈에 맞춰 과거 4개 seed와 새 실패 seed를 현재 경로에 보존했습니다.

Qwen 생성·편집·Fill·제거, 원본 latent 마스크와 흑백 가이드 제거 방식, HyMT, 비파괴 레이어/Undo/취소, 메타데이터, AVIF 및 설치 핀은 변경하지 않았습니다.

### 빌드와 검증

- 후보 소스: `953cfe44f9a35a6aec1a3c1836c30f4f2bdf5bd2`
- 태그/main 병합 소스: `06c7aa3073df2be65b7bb06d7293a1e4d30b1d34`
- upstream v0.20.1: `327c4cc82e737a3e0af7c147a477126d77525495`, 릴리스 ID `432155025`
- Windows x64 MSVC, Rust 1.95, static CRT, AVIF portable ZIP
- 후보 전체 CI, Qwen 회귀 804건, Windows ARM64 설치/실행 및 [병합 후 main CI](https://github.com/david419kr/photocraft-qwen-edit/actions/runs/41600064325) 통과. main 테스트: Ubuntu 14,094건, Windows 단위 11,229건·통합 2,768건, corpus 5,890건. ignored/skipped와 상세 결과는 `release-provenance.json`에 구분해 기록했습니다.
- ZIP SHA256: `86a7c31139abd641e795f6a140935519f524c44008f39e55da035559046fad53`

CPU/GPU 비교 회귀 테스트는 클라우드의 소프트웨어 그래픽 백엔드에서도 실행했습니다. 실제 하드웨어 GPU·CUDA·VRAM·ComfyUI 모델 추론·대화형 UI 검증은 수행하지 않았으며 사용자 PC에 접근하지 않았습니다. Sonar 실제 분석은 토큰 부재로 실행되지 않았습니다. FreeBSD는 기존 정책대로 수동 검사이며 이번 릴리스에서는 실행하지 않았습니다.

조사 중 확인된 별도의 U8 반올림 경계에서 발생하는 한 단계 차이는 이번 Fill 수정의 해결 범위에 포함되지 않습니다. 자세한 근거와 범위는 [조사 기록](https://github.com/david419kr/photocraft-qwen-edit/blob/953cfe44f9a35a6aec1a3c1836c30f4f2bdf5bd2/docs/cmyk-adjustment-fill-fix.md)을 참고하세요.
