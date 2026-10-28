# PhotoCraft Qwen AI 생성형 편집 통합 개선안

> 작성일: 2026-10-28 · 설계 v1 · 기준 코드: `665fa4eb0029e07ccd02620540f47952edc44f40`
>
> 상태: **통합 구현 및 검증 기록**. Prompt Edit·Generative Fill 개선을 적용했다. 2026-10-28 사용자 결정으로 Qwen 타일링 배경 제거는 현재 옵셔널 API 구현을 완료 범위로 확정하며, 추가 탐구와 자동 활성화를 진행하지 않는다.

> 구현: 공통 frame budget·RGBA·참조1장·승인 resize의 atomic commit, variable crop·독립 Remove 그룹·고정 입력 재생성·crop별 Poisson을 연결했다. 타일링은 `experimentalRefine:true`로만 호출하며 기본 UI는 단일 경로다. 초기 alpha를 sampler에 고정하고 낮은 노이즈로 경계·세부 구조만 정제하며, 작업 범위 밖 native coarse alpha와 원본 raster를 보존한다. 품질 이득과 비용이 사례별로 달라 자동 적용하지 않는다. 아래 검증 매트릭스는 설계·회귀 기준이며 모든 실제 이미지의 화질을 보증하지 않는다.

## 1. 목표와 개발 원칙

1. ComfyUI를 모르는 사용자도 PhotoCraft 안에서 이미지 첨부와 자연어만으로 편집할 수 있어야 한다.
2. node, latent, mask 반전, 내부 pixel budget, 타일 설정을 새 사용자 설정으로 노출하지 않는다.
3. 기존 정상 사례의 화질, 원본 보존, 취소, Undo/Redo, 재생성, 수동 Poisson 보정을 유지한다.
4. Qwen Image 2.1과 설치된 표준 ComfyUI 노드를 우선 사용한다. 전용 inpainting 모델·새 가중치·custom node·런타임 의존성을 기본 전제로 추가하지 않는다.
5. 최신 코드와 실제 graph를 확인한 뒤 기능별로 구현·검증한다. 서로 다른 변화의 화질 원인을 구분할 수 있도록 한꺼번에 전환하지 않는다.
6. UI는 얇게 유지하고 새 동작은 engine command와 자동화 경로에서도 실행 가능해야 한다. Rust 계층·wasm·never-crash 규칙을 따른다.
7. GPU 모델명이나 고정 adapter index로 제품 동작을 결정하지 않는다. 기존 backend의 장치 선택과 실제 상태 보고를 사용한다.
8. fake-backend 회귀 테스트와 실제 모델의 화질 검증을 분리한다. 이번 환경의 실기 검증은 사용자 지시에 따라 **8060S에서 Turbo 4~6step, 최소 횟수**로 제한한다. 이것을 제품 기본값 변경이나 공식 품질 평가 조건으로 간주하지 않는다.

### 1.1 확정 요구와 설계 초기값

- **확정 요구:** 사용자가 승인한 동작 계약. 구현 시 유지해야 한다.
- **설계 초기값:** 구현·대표 실험을 시작할 구체적 수치. 성능·품질 결과에 따라 문서와 함께 조정할 수 있으며 아직 검증된 제품 상한이 아니다.
- **검증 필요:** 구조적으로 연결 가능하더라도 모델 의미 이해·경계 정확도·모든 재질의 품질까지 확인한 것은 아니다.

## 2. 작업 종류와 원본 보존 계약

Qwen의 image-edit 추론을 사용한다는 이유로 모든 기능을 동일한 Prompt Edit 작업으로 취급하지 않는다.

| 작업 | 입력 대상 | 최종 결과 | 문서 리사이즈 |
|---|---|---|---|
| Prompt Edit | 현재 visible 문서 합성 + 선택적 참조1장 | 생성 RGB·alpha의 결과 레이어 | 예산 초과 시 사용자 승인 후만 허용 |
| Generative Fill | 원본 snapshot의 ROI + 모델용 선택 guide | 원래 selection coverage로 제한한 생성 레이어 | 하지 않음 |
| Background Removal | 활성 raster 레이어 자체 | 원본 픽셀에 적용할 editable LayerMask | **항상 하지 않음** |
| Prompt Generate | 기존 생성 경로 | 기존 생성 결과 | 이번 개선의 동작 변경 대상 아님 |

공통으로 재사용할 부분은 모델 조회·로딩, ComfyUI 통신, CMS, 이미지 운반, 취소·job 관리, 기하 연산이다. 입력 선택·해상도 정책·결과 적용·history 계약은 작업 종류별로 분리한다.

### 2.1 원본 보존의 정확한 의미

- Fill: 최종 selection coverage가0인 곳은 원본 픽셀 불변. 사용자 feather를 한 번만 적용한다.
- Background Removal: 문서 크기·원본 raster의 색 채널·원래 alpha·depth·profile을 보존하고 mask만 바꾼다. 기존 layer mask 교체는 한 번의 Undo로 복원한다.
- Prompt Edit: 기존 원본 레이어는 유지하되 생성 RGB·alpha를 결과로 사용한다. **사용자가 승인한 Image Size는 원본 레이어 픽셀도 의도적으로 resample하는 예외**다.
- 승인 취소, 추론 실패, 중간 취소, 응답 오류, 대상 충돌이면 문서·dirty 상태·Undo/Redo를 원래대로 유지한다.
- 원본 raster 보존은 ‘완벽한 alpha 추정’ 또는 ‘경계에 섞인 배경색까지 무손실 제거’를 의미하지 않는다.

## 3. 공통 해상도 계획

### 3.1 공식 native2K 기준

Qwen 공식 권장값은 다음과 같다. 긴 변2048의 절대 상한이 아니다.

| 명목 화면비 | 권장 크기 | 픽셀 수 |
|---|---|---:|
|1:1|2048×2048|4,194,304|
|4:3|2400×1792|4,300,800|
|3:4|1792×2400|4,300,800|
|3:2|2528×1696|4,287,488|
|2:3|1696×2528|4,287,488|
|16:9|2752×1536|4,227,072|
|9:16|1536×2752|4,227,072|

약4.2MP를 엄격한4,200,000px 또는2048²로 구현하면 일부 공식 권장값을 거부한다. **표 전체를 수용하는 패딩 포함 예산**으로 정의한다. 표에 없는 비율도 원본 화면비로 계산하며, 명목 비율이나 표 크기에 맞추기 위해 이미지를 찌그러뜨리지 않는다.

### 3.2 수치 정책

| 항목 | 값/규칙 | 성격 |
|---|---|---|
|표준 목표 면적|2048² =4,194,304px|계산 기준|
|최대 padded model 면적|4,300,800px 포함|공식 권장표를 수용할 기본 계약|
|model padded 각 변 안전 상한|초기안4096px|검증 전 초기값; 모델 architecture의 공식 최대치라는 뜻 아님|
|Fill 최소 working 긴 변|512px|기존 의미 유지; 작은 문서 예외 포함|
|첨부 reference 예산|초기안1024² padded px|원본과 독립; 실제 identity/detail 검증 후 조정|
|분할 Fill 그룹 수|초기안 최대4개|불필요한 요청 폭증 방지|
|배경 제거 정제 타일 수|초기안 최대16개|긴 작업·중복 추론 방지; 범위 검증 필요|
|기존 AI 원본문서 안전 범위|초기 구현은 기존36MP guard 유지|4.3MP 모델입력 예산과 별개; 더 큰 문서 지원은 별도 자원 검증|

극단적 화면비는 면적만으로 안전하다고 판단하지 않는다. padded 각 변 상한과 정수/할당 범위를 별도로 검사한다. 추가 padding·안내·거부 중 어떤 정책으로 처리할지는 대표 극단 비율 테스트 후 확정한다. 어느 경우에도 문서의 원래 종횡비를 강제로 바꾸거나 조용히 crop하지 않는다.

### 3.3 계산과 기하 불변식

해상도 계획은 적어도 다음을 구분해 보관한다.

- native 문서 크기와 native 작업 ROI.
- 실제 이미지 content 크기와 padded model frame 크기.
- frame 안의 content rect, padding의 방향·폭, source/model 간 축별 scale.
- 픽셀 중심 좌표 규약, ROI 원점, 역변환 및 복원 filter.

규칙:

1. padded 크기는32배수다. 모든 padding을 적용한 뒤 면적·각 변 상한을 다시 검사한다.
2. 허용 범위 안의 원본은 content 해상도를 그대로 유지한다. 작은 입력을 무조건2K로 확대하지 않는다.
3. 원본이 큰 경우 종횡비를 유지해 목표를 계산하고 padding 후 상한을 만족하도록 조정한다.
4. content 크기와 문서 크기를 padding 때문에 같게 만들지 않는다. 결과는 content rect만 추출한다.
5. reference마다 화면비·예산을 독립 적용한다. 원본과 reference를 같은 크기로 강제하지 않는다.
6. source/frame/guide의 좌표 계약을 함께 고정한다. 임의의 전역 반픽셀 보정이나 영상 내용 기반 정합 이동을 추가하지 않는다.
7. 모델이 더 큰 입력을 받을 수 있다는 것과 이 PC에서 실행 가능하다는 것은 별개다. 메모리 실패 시 승인 없이 문서 크기를 다시 줄이거나 다른 모델로 바꾸지 않는다.

출력 frame 예산 외에 **원본+첨부+전체 문맥+guide의 합산 conditioning 비용**도 별도로 검사·측정한다. image1이4.3MP 안이라는 이유로 여러 조건 이미지의 메모리까지 안전하다고 간주하지 않는다. 이 검증을 마치기 전에 모든 reference를 native2K로 확대하지 않는다.

## 4. Prompt Edit 개선

### 4.1 선택적 reference1장

확정 UX:

- 기존 확장 Prompt Edit 입력줄에 첨부 버튼을 추가한다.
- 첨부 후 작은 thumbnail과 제거 동작을 제공한다. 좁은 창에서도 기존 조작을 불필요한 추가 행으로 밀지 않는다.
- 사용자가 자연어로 참조의 활용법을 지정한다. 스타일/인물/구도 같은 별도 방식 설정 UI는 만들지 않는다.
- 미첨부 상태의 기존 동작을 유지한다.
- Fill·배경 제거로 전환했을 때 Prompt Edit의 첨부가 숨겨진 입력으로 따라 들어가지 않는다.

입출력 계약:

- image1은 편집할 원본, image2는 첨부 reference다. 출력 latent/frame도 image1 기준이다.
- 내부 role 안내가 필요하면 원본과 reference를 구분하되 특정 대상 종류나 편집 의도를 강제하지 않는다.
- UI의 user prompt 원문과 내부 role 지시를 구분해 history에 기록한다.
- reference는 자신의 ICC/alpha를 바르게 처리한 뒤 모델용 사본으로 변환한다. 사용자 원본 파일은 수정하지 않는다.
- 최초 구현의 범위는 같은 세션의 작업과 재생성이다. 문서 재열기 후에도 reference를 영속 보관하는 기능은 별도 범위다.

수명:

- 제출 시 source/reference의 bytes·처리 크기·역할을 snapshot으로 고정한다. 파일 경로만 저장하지 않는다.
- 진행 중 첨부 변경·제거·원본 파일 삭제가 현재 작업이나 이전 작업 재생성을 바꾸면 안 된다.
- UI 사본, 진행 job 사본, LastRun 사본의 소유권을 구분한다. 필요 소유자가 없어지면 메모리와 thumbnail cache를 해제한다.
- Comfy 업로드 사본과 로컬 snapshot 정리는 별개다. 설치본의 temp upload/annotated path와 지원 API를 확인해 사용자 원본이 아닌 **자신의 작업 사본만** 정리한다. temp 수명에 의존하는 부분을 즉시 삭제로 과장하지 않는다.
- CLI/control/MCP에서도 첨부·제거·실행을 drive할 수 있어야 한다. filesystem 접근은 기존 capability 경로를 사용한다.

### 4.2 문서 자체 resize

긴 변1536까지 줄여 생성한 뒤 이전 문서 크기로 재확대하는 현재 Prompt Edit 정책을 교체한다.

허용 범위 안이면 문서 content 크기로 생성한다. 초과하면 **현재 문서 크기와 변경될 문서 크기**를 보여주고 승인을 받는다. node·padding·latent 용어는 안내에 넣지 않는다. 취소하면 아무 문서 변경도 없다.

승인 및 commit:

1. preflight 결과를 문서ID·revision·원본크기·target크기에 연결한다.
2. 승인 시 같은 대상인지 확인한다. 그 사이 문서가 바뀌었으면 오래된 승인으로 실행하지 않는다.
3. worker의 복제 Document에서 기존 Image Size 공통 구현으로 resize한다.
4. 축소된 문서의 composite를 AI 입력으로 사용한다.
5. 생성 결과는 그 문서 크기에 적용한다. padding을 제거할 뿐 이전 크기로 확대하지 않는다.
6. 성공 시 resized 문서+AI 결과+관련 세션 기하 상태를 한 번에 commit한다.

현재 문서에 `image.imageSize`를 먼저 적용하고 실패 시 `edit.undo`로 되돌리는 방식은 쓰지 않는다. 기존 `resample_doc`와 **크기·할당 budget 검사**를 함께 재사용한다. 레이어·마스크·채널·selection·vector/text·guide·effects·smart-object cache뿐 아니라 colour-sampler 등 Session/UI 부수 상태도 성공 시에만 같은 변환을 반영한다.

Undo1회로 resize 이전 문서를 복원하고 Redo1회로 축소+편집 결과를 복원한다. 재생성은 성공한 target 크기와 resized pre-edit snapshot을 재사용하며 resize를 중복 실행하지 않는다.

### 4.3 실제 RGBA 입력·출력

현재 Prompt Edit의 출력 alpha는 이미 보존된다. 개선 중심은 흰색에 flatten하고 alpha255를 강제하는 **입력 경로**다.

- 투명 원본은 실제 alpha를 보존한 sRGB RGBA 사본을 전달한다.
- `LoadImage`의 RGB와 MASK를 `JoinImageWithAlpha`로 결합해 `TextEncodeQwenImage21`에 넣는다.
- 설치본에서 `LoadImage.MASK=1-alpha`, Join은 내부에서 다시 반전한다. 직접 연결하고 추가 반전을 하지 않는다.
- vision tower만 흰색 composite를 보는 것은 Comfy 노드 내부 동작이다. VAE에 전달할4채널까지 flatten하지 않는다.
- reference의 alpha도 독립적으로 같은 계약을 적용한다.
- 불투명·무참조 경로는 불필요하게 바꾸지 않는다.
- 결과 RGB와 alpha를 기존 CMS/문서 depth로 결과 레이어에 반영한다. 모델 운반·응답은 현재 sRGB8/RGBA8 PNG라는 점을 유지한다. 16/32비트 문서 지원을16/32비트 생성 정밀도라고 표현하지 않는다.

## 5. Generative Fill 개선

### 5.1 가변 화면비 crop

- 선택 bbox 화면비를 그대로 강제하지 않고, 복원에 필요한 각 축의 문맥을 확보한다.
- 정사각형, 가로형, 세로형, 매우 얇은 선택, canvas edge의 부족한 문맥을 다룬다.
- 기존 square crop이 이미 적절한 정상 사례는 가능한 한 유지하는 보수적 전환을 우선 검토한다.
- 작은 ROI는 native scale을 유지하거나 기존 최소 작업 크기까지 필요한 만큼만 확대한다.
- 큰 ROI만 공통2K 면적 예산으로 축소한다. 긴 변2048을 절대 상한으로 쓰지 않는다.
- crop 이동·원점·padding·비정수 scale·역변환을 하나의 고정 계획으로 관리한다.
- 문서 자체는 resize하지 않는다.

### 5.2 마스크 계약 유지

다음 데이터는 서로 대체하지 않는다.

| 데이터 | 의미 |
|---|---|
|source alpha|실제 원본 이미지의 opacity|
|model guide|모델이 편집할 곳을 알려주는 입력|
|final coverage|원래 문서 좌표의 사용자 selection/feather|
|reference alpha|참조 이미지 자체의 opacity|
|generated cutout alpha|배경 제거가 추정한 최종 opacity|

현재 Fill은 업로드 alpha를 `1-modelGuide` 운반용으로 사용한다. Prompt Edit용 RGBA Join을 무조건 공통 적용하면 선택 영역이 실제 투명도처럼 VAE에 들어가 기존 동작이 바뀐다. Fill의 source latent/noise mask 제약과 원래 RGB 참조, 별도 흑백 guide를 유지한다.

얇은 선택·작은 점·구멍·약한 feather를 삭제하지 않는다. 최종 selection coverage를 팽창하거나 새 feather를 더하지 않는다. coverage0인 곳은 모든 문서 depth/profile에서 원본 픽셀을 유지한다.

### 5.3 빈 prompt Remove의 독립 영역 분할

분할은 **빈 prompt의 Remove만** 대상으로 한다. 일반 prompt가 있는 Fill은 기존 단일 요청을 유지한다.

1. selection 연결 성분과 공간 분포를 분석한다.
2. 글자 획·점 등 근접한 작은 성분을 공간 그룹으로 묶는다.
3. 그룹별 필요한 context crop, 중첩, 분할 후 유효 해상도 이득과 요청 수를 함께 평가한다.
4. 충분히 독립적일 때만 분할하고, 이점이 작거나 애매하면 단일 요청을 사용한다.
5. 그룹 수 상한을 적용하고 plan을 deterministic하게 고정한다.

실행은 우선 순차 요청으로 구현한다. 모든 요청은 **동일한 최초 문서 snapshot**을 사용하며 앞 결과를 다음 입력으로 사용하지 않는다. 같은 모델 weights를 재사용하고 불필요한 중복 추론을 피한다.

모든 결과를 받은 뒤 group별 원래 coverage에만 적용해 최종 레이어1개·Undo1개를 만든다. 일부 실패·취소 시 부분 레이어나 부분 history를 적용하지 않는다. 자신의 job만 취소하며 Comfy의 전역 interrupt나 다른 작업의 queue를 건드리지 않는다.

연결 성분 판정용 임시 core와 최종 soft coverage는 구분한다. feather의 미세 값이 성분을 이어주거나 support가 겹치는 경우, coverage를 버리거나 두 번 blend하지 않는다. 소유권이 애매하면 그룹을 합치거나 단일 요청으로 fallback한다.

### 5.4 재생성과 Poisson

현재 request/region 하나인 LastRun과 original/generated/region 하나인 Poisson Source를 그룹 plan으로 확장한다.

- group별 source, ROI, content/padded 크기, guide, 원시 생성 결과, seed, 복원 filter를 보관한다.
- 재생성은 최초 그룹 구성·원본·설정을 유지하고 새 root seed에서 안정적인 group seed를 만든다.
- Poisson은 각 crop의 실제 surrounding pixels로 계산한 뒤 같은 합성 plan을 사용한다. 임의의 전체 빈 bbox를 하나의 생성 frame처럼 취급하지 않는다.
- strength0은 최초 합성 결과와 같아야 하고, 반복 적용해 보정을 누적하지 않는다.
- preview/취소/한 번의 apply Undo/Redo/레이어 우클릭 재진입을 유지한다.
- 완료 직후 하단바 제안은 기존 Remove 자리만 일시적으로 사용한다. 다른 동작 뒤에는 사라지고 이후 접근은 레이어 우클릭이다. 추가 행이나 `…`를 다시 넣지 않는다.
- Poisson buffer 검증도 side²가 아닌 공통 pixel budget을 사용한다. rectangular 공식 권장 크기를 허용한다.

## 6. 배경 제거: 독립 Mask 작업

### 6.1 기본 동작

배경 제거는 사용자 관점과 작업 계획에서 별도 operation으로 둔다. 내부에서 Qwen image-edit 추론을 쓰더라도 Prompt Edit의 문서 resize·첨부 reference·생성 RGB 적용 정책을 상속하지 않는다.

```text
활성 native raster snapshot ─────────────────────→ 원본 픽셀 유지
          └→ Qwen 배경 제거 → 생성 alpha → native LayerMask
```

- 원본 raster와 문서 크기를 유지한다. 모델 입력을 축소해도 문서를 축소하지 않는다.
- 생성 RGB는 사용하지 않는다. 타일 처리에서도 RGB patch를 조합하지 않는다.
- 원본 layer ID, blend/opacity/styles와 다른 레이어 visibility를 유지한다.
- 기존 layer mask는 현행처럼 교체하고 Undo로 복원한다. Background는 mask를 받을 수 있는 normal layer로 전환한다.
- 기존 선택 영역으로 처리 범위를 몰래 제한하지 않는다.
- 현재 검증된 **대상 종류를 가정하지 않는 범용 RGBA 프롬프트**를 기준으로 한다. 인물·사진·캐릭터만 전제하지 않는다.
- 자동 Poisson을 적용하지 않는다. 경계색 오염 제거는 별도의 선택적 후처리로 검토하며 기본 원본 RGB 불변과 구분한다.

### 6.2 고해상도: 전체 판단 후 필요한 영역 정제

독립 타일마다 ‘배경을 제거하라’고 요청한 뒤 단순 평균하는 방식은 기본안으로 쓰지 않는다. 잘린 일부만 보면 전경 판단이 바뀌거나 같은 곳을 서로 다르게 분류할 수 있기 때문이다.

권장 구조:

1. 원본 layer 전체를 budget 안의 사본으로 처리해 coarse alpha와 전체 문맥을 얻는다.
2. 원본 해상도에 투영한 coarse 결과와 source detail을 바탕으로 정제할 영역을 계획한다.
3. 원본 크기의 overlap tile/ROI를 요청하고, 전체 문맥과 coarse guide를 필요한 방식으로 전달한다.
4. tile별 RGB는 버리고 raw alpha만 원본 좌표로 복원한다.
5. 겹침과 판단 불일치를 처리해 단일 native mask를 완성한다.
6. 전부 성공한 뒤 원본 layer의 mask를 한 번에 교체한다.

이 계획의 추가 참조는 내부 문맥 자료다. Prompt Edit UI의 사용자가 첨부한 reference와 혼동하거나 자동 재사용하지 않는다. `image1=출력할 tile`, `image2=전체 문맥`, `image3=coarse guide` 같은 역할 구성은 **실험할 설계안**이며 아직 Qwen에서 품질이 검증된 refinement API가 아니다. 원본 RGB에 표시색을 덧칠하는 방식을 기본으로 삼지 않는다.

### 6.3 정제 영역 계획과 상한

- 경계 band를 우선하되 머리카락·가는 선·작은 구멍·작은 독립 부품 등 coarse 축소에서 사라질 수 있는 구조도 고려한다.
- coarse alpha는 opacity이지 보정된 confidence score가 아니다. 값이0/255에 가깝다는 이유만으로 언제나 배경/전경이 확실하다고 간주하지 않는다.
- 단순한 내부/외부 영역은 재추론을 줄일 수 있지만, 그 생략 정책의 누락 위험을 검증한다.
- tile 수·총 padded pixels·예상 최대 작업량을 계획 단계에서 제한한다. 겹치는 불필요한 ROI를 합친다.
- 상한을 넘었을 때 영역을 조용히 누락하거나 저품질 coarse mask를 고해상도 완성 결과로 표시하지 않는다. 사용자에게 간단한 실패/지원 범위 안내를 주고 문서는 유지한다. fallback 품질 선택 UI는 별도 요구가 없는 한 추가하지 않는다.
- 처음에는 기존36MP 원본문서 안전 범위에서 검증한다. 더 큰 문서의 지원은 native mask 할당·history·streaming·취소 시간 실측 후 별도로 확장한다.

### 6.4 overlap 초기 실험값과 합성

초기안은 입력 frame 최대2048², 사방 문맥 halo256px, 유효 core1536²다. 이웃 core를128px 겹치면 stride는1408px가 된다. **이는 검증 전 출발값이며 Photoshop에서 확인한 tile 공식이라는 뜻이 아니다.** canvas edge의 clipped ROI와 padding은 따로 기록한다.

유효 영역의 중심에 높은 가중치를 주고 겹침에서 부드럽게 전환한다. 같은 native 픽셀의 tile 예측 `a_i`와 가중치 `w_i`는 다음처럼 정규화할 수 있다.

`a(x) = Σ(w_i(x) × a_i(x)) / Σw_i(x)`

- 전체 적용 대상에서 분모가0인 구멍이 없어야 한다.
- 여러 mask를 alpha-over로 반복 합성하거나 opacity를 반복 곱하지 않는다.
- 타일마다 새 feather를 넣지 않는다. overlap용 weight는 사용자 mask feather와 다른 개념이다.
- 한 타일은0, 다른 타일은1인 강한 불일치를 단순 평균해0.5로 감추지 않는다. 전체 판단·더 넓은 문맥·국소 재검증/merge 등의 bounded 처리로 구분한다.
- 실제 반투명 재질의 중간 alpha를 타일 불일치로 오인해 무조건0/1로 만들지 않는다.
- raw alpha 정규화/threshold로 모델 실패를 성공처럼 보이게 하지 않는다. 필요 보정은 별도 실험과 계약이 있어야 한다.

### 6.5 기존 원본 alpha와 mask의 결합

원본이 이미 RGBA인 경우의 이중 곱셈을 검증해야 한다. 원본 alpha가0.5인데 생성된 최종 alpha0.5를 그대로 layer mask로 넣으면 최종0.25가 될 수 있다.

확정 불변식:

- 원래 raster alpha는 수정하지 않는다.
- 의미가 최종 opacity인 생성 alpha와, raster alpha에 곱할 상대 mask coverage를 구분한다.
- 원래 alpha0인 곳에 새 foreground를 만들어내지 않는다.
- 원래 alpha를 두 번 적용하지 않는다. 기존 layer opacity·vector mask 등과의 결합도 확인한다.

설계안으로는 원본 alpha `s`, 목표 최종 opacity `t`에 대해 `0 ≤ t ≤ s`를 유지하고, `s>0`이면 상대 mask를 `t/s`로 변환할 수 있다. zero·작은 float 값·비정상 값은 명시적으로 처리한다. 불투명 원본에서는 `s=1`이므로 현재의 raw alpha 적용과 같다. 이 계약은 source RGBA 입력과 실제 반투명 예제로 검증한 뒤 확정한다.

### 6.6 ‘손실 없음’의 범위

타일링으로 원본 RGB의 축소·생성·재확대를 피할 수 있다. 다만 다음을 보장한 것은 아니다.

- 모델이 모든 미세 경계·구멍·반투명 재질을 정확히 추정함.
- native 해상도 tile 출력이 source의1px 구조를 반드시 복구함.
- 원래 배경색이 섞인 경계 RGB가 mask만으로 완전히 제거됨.

경계색 복원이 필요하면 원본을 유지한 별도 correction 방식으로 설계해야 한다. 기본 배경 제거를 색상 재생성 작업으로 되돌리지 않는다.

## 7. 공통 작업·상태·재생성 구조

아래는 역할을 나타내는 설계명이며 현재 존재하는 API명이라고 가정하지 않는다.

| 계획 데이터 | 보관할 내용 |
|---|---|
|Operation|Prompt Edit / Fill / Background Removal 목적|
|Source snapshot|doc ID, revision, active layer, native doc, 원래 alpha/selection/profile|
|Resolution plan|content/padded 크기, content rect, ROI, scale, filter, frame budget|
|Reference snapshot|역할, immutable image bytes, 처리 크기, content hash, 수명|
|Pass plan|단일 편집, Fill group, cutout coarse/refine, seed, 설정, 자기 job UUID|
|Result record|원시 결과, region별 기하, 최종 layer/mask baseline, 재생성·Poisson 입력|

상태는 최소한 준비→승인 대기→실행→결과 수집/검증→commit→완료로 구분한다. 실패·취소는 commit 전 종료이며 다른 작업의 history를 Undo하지 않는다.

- 모델 실행 동안 source 문서 snapshot과 목표를 고정한다.
- tab을 바꿔도 원래 문서에만 반영하고 사용자가 보던 문서를 불필요하게 바꾸지 않는다.
- source가 닫히거나 revision이 충돌하면 결과를 폐기한다.
- 응답 지연·연결 끊김 시 같은 생성 POST를 무작정 재전송하지 않는다. 중복 생성을 피한다.
- reroll은 현재 UI draft가 아니라 고정된 최초 plan·reference·설정을 사용한다.
- 작업 목적도 제출 시 고정한다. resize 뒤 selection 분류나 UI draft가 달라졌다는 이유로 Prompt Edit을 Fill 등 다른 기능으로 재해석하지 않는다.
- history·dirty 상태·선택 대상·layer ID·prompt history 기록을 일관되게 적용한다.
- 오래된 source 기록이 retouch를 덮어쓰지 못하도록 현재 mask/pixel 상태 검증을 유지한다.

## 8. 현재 코드의 재사용 및 변경 범위

| 영역 | 현재 재사용점 / 필요한 확장 |
|---|---|
|`engine/src/ai_cmds.rs`|command, immutable source, LastRun, 결과 적용. 목적별 policy와 batch plan 분리|
|`engine/src/ai_cmds/fill.rs`|ROI·guide·native final coverage·보수적 thin selection 보존. variable crop/면적 예산으로 확장|
|`engine/src/ai_cmds/cutout.rs`|원본 layer 보존+alpha-only mask. 독립 목적·RGBA·coarse/refine plan 확장|
|`engine/src/ai_cmds/harmonize.rs`|수동 Poisson·preview·strength0·retouch 보호. per-pass source와 공통 면적 검증 확장|
|`engine/src/comfyui.rs`|표준 노드 graph·모델 준비·PNG·job별 cancel. multiple input/RGBA/면적 제한 확장|
|`engine/src/image_cmds.rs`|Image Size의 `resample_doc` 및 사전 크기·할당 검증 재사용|
|`engine/src/jobs.rs`|snapshot 검증·문서별 적용·취소·atomic edit 재사용|
|`algo/src/segment/mod.rs`|기존 연결 성분 분석 재사용 검토. 큰 mask의 할당과 취소는 추가 확인|
|`ui-egui/src/ai_bar.rs`, `state.rs`|기존 prompt UI·state. reference thumbnail/제거·resize 승인과 자동화 연결|

현재의1536 상수, Comfy의 양쪽 변 검사, `result_layer` 검사, Poisson의 side² 검사, cutout 검사까지 함께 점검한다. 상수 하나만2048로 바꾸거나 Prompt Edit 정책을 모든 작업에 그대로 적용하지 않는다.

## 9. 사용자에게 보이는 변화

- 평소에는 기존 원클릭 작업 흐름을 유지한다.
- Prompt Edit 확장 입력줄에서만 선택적 참조 첨부와 thumbnail/X를 제공한다.
- 원본 문서 축소가 필요한 Prompt Edit에서만 전후 크기와 승인/취소를 보여준다.
- Fill과 배경 제거는 내부 분할·좌표·node를 노출하지 않는다. 필요할 때 ‘영역2/4’ 같은 일반 진행 상태만 표시한다.
- 내부 예외·메모리 실패·지원 범위는 문서가 보존됐음을 포함해 간단히 설명하고 상세 진단은 자동화/log로 분리한다.
- 아직 붙이지 않은 Adobe backend나 고급 tile/threshold 설정을 미리 UI에 추가하지 않는다.

## 10. 구현 단계와 완료 조건

| 단계 | 작업 | 완료 조건 |
|---|---|---|
|0|기준점 고정·현재 경로와 정상 사례 기록|commit/테스트/대표 결과 추적 가능|
|1|목적별 공통 해상도·기하·mask 의미 분리|공식 preset·padding·극단 비율·타 작업 비침범 테스트|
|2|Prompt Edit RGBA|불투명 경로 회귀 없음, 투명 source/reference 운반·출력 검증|
|3|참조1장 UI·snapshot·수명|첨부/제거/재생성·원본/참조 역할·취소·자동화 검증|
|4|Prompt Edit 승인 resize와 atomic commit|1Undo, 실패/취소 시 doc/history/dirty 불변, 복원 확대 없음|
|5|단일 Fill variable crop·native2K|mask 밖 불변·기하·Poisson·reroll 회귀 없음|
|6|빈 prompt Fill 그룹 분할|전부 성공 후 layer1개·Undo1개, per-group reroll/Poisson|
|7|배경 제거 독립화·coarse/refine 최소 실험|문서/색 픽셀 불변, 문맥 전달 효과·호출 비용 확인|
|8|배경 제거 overlap tile 합성|cover/불일치/기존alpha/미세구조·취소·상한 검증|

기능별 fake 테스트가 통과한 뒤 해당 기능의 최소 실기 검증을 한다. 해상도·crop·prompt·reference·분할을 동시에 바꾼 결과로 원인을 단정하지 않는다. 공개 동작 전환은 해당 단계의 기준을 만족한 뒤 한다.

## 11. 자동화 검증 매트릭스

| 구분 | 필수 사례/불변식 |
|---|---|
|기존 회귀|Prompt Edit/Generate/Fill/cutout, no-reference·opaque graph, 모델 설정·prompt history·reroll|
|해상도|공식7preset, 예산 바로 아래/위, padding 후 초과, odd size, native≤예산, 매우 작은 문서, 극단 비율·각 변 상한|
|resize|승인·취소·오래된 승인, resize+AI 단일Undo/Redo, 실패/취소 후 redo stack·dirty·sampler 좌표 불변|
|참조|없음/1장, 서로 다른 크기·화면비·ICC·alpha, 중간 첨부 변경·제거·파일 삭제, document 전환, temp 정리|
|RGBA|alpha0/부분/1, 흰색/검정/유색 경계, 실제 source alpha와 guide 반전, opaque branch 일치|
|Fill 기하|정사각·가로·세로·thin/1px/tiny dots/구멍/feather/경계, 비정수scale·ROI원점·32padding·역변환|
|Fill 분할|가까운 글자 그룹, 먼 영역, 겹친 context, feather bridge/support 충돌, 모호하면 단일 호출, 그룹 수 상한|
|Fill 결과|원래 selection surface 값 유지, coverage0 pixel 불변, 모든 요청 같은 source, layer1개·Undo1개|
|수동 보정|single/batch Poisson preview·취소·strength0·반복 적용·retouch 방어·UI 재진입 규칙|
|cutout|native RGB/원래alpha/profile/depth/layer ID/다른 visibility 불변, 기존mask 교체·background 전환·empty/malformed 출력|
|cutout tile|coarse/정제 영역 누락, core+halo 좌표, overlap 가중치 합, 경계 구멍, tile 불일치, 실제반투명, alpha 이중곱 방지|
|job 수명|부분 실패·취소·네트워크 오류·tab 전환·document 종료·동시편집 충돌, 중복 POST/전역 cancel 금지|
|형식/안전|RGB/Gray/CMYK/Lab,8/16/32bit, finite/범위/할당 검증, lock/parent lock, never-crash·wasm·계층|

fake backend는 요청 입력/기하/guide/참조와 의도적으로 다른 generated RGB·alpha를 반환해 원본 보존과 결과 적용을 검증한다. fake 결과가 맞는다는 이유로 실제 모델의 전경 판단·머리카락 품질을 합격 처리하지 않는다.

## 12. 실기 검증

- Prompt Edit: 참조의 유무/역할, RGBA 입력 보존, 작은 입력 유지, 기존1536과새정책의 결과.
- Fill: square/가로/세로, 작은 정상 ROI 회귀, 먼 선택의 단일/분할, 주변 선·텍스처·반투명 overlay의 연속성.
- cutout: 인물·비인물·일러스트·가는 선·구멍·밝고어두운 표면·반투명 재질. 인물만 전제하지 않는다.
- 고해상도 cutout: 전체 축소 mask와 coarse+tile 결과를 비교한다. 원본 크기에서 경계 누락·halo·patch 불일치·작은 부품을 확인한다.
- 합성 시료는 ground truth alpha로 수치 평가하고 실제 사진·그림은 진단 patch와 시각 비교를 병행한다. GT 없는 이미지에서 정합 오차0/전체 accuracy를 선언하지 않는다.
- peak memory·모델별 호출 수·총시간·준비/추론/전송 구간을 구분한다. 모델 해상도와 document 크기를 혼용하지 않는다.
- 8060S 실험은 Turbo4~6step·최소 횟수다. 품질 일반화나 공식8step/기본모델 대비 우열 판정에 쓰지 않는다.

## 13. Adobe 로컬 모델 연구의 위치

별도 로컬 연구에서 설치 Photoshop의 피사체 모델·정제 모델은 독립 CPU/GPU 세션 준비와 각각 합성 tensor의 DirectML Run1회까지 확인했다. **실제 이미지의 전체 전처리·고해상도 ROI 조립·최종 배경 제거 품질·PhotoCraft 제품 연동이 완료된 것은 아니다.**

추가 관측에서는 전체 입력의 긴 변1024 fit/center padding, coarse 기반 ROI의 여백, 큰 ROI의 전체+4개 local 정제창,1024창/256overlap/768stride를 확인했다.6000×4000 합성 문서에서4478×3815 ROI를1792²로 처리한 뒤 원래 위치의 layer mask로 복원하는 동작도 확인했다. 큰 ROI에는 별도의 원본 RGB-guided CPU 단계도 관측했다. 이는 로컬 연구 시료의 관측이며 모든 조건의 공식을 확정한 것은 아니다. 근거는 `plan/2026-10-28-background-removal/adobe-direct/high-resolution/README.md`에 있다. 이 문서의 Qwen tile 초기값은 별도 가설이며 Photoshop의 공식으로 부르지 않는다. proprietary 코드·가중치를 제품에 복사하지 않는다.

이 연구는 Qwen 개선의 선행 의존성이 아니다. 나중에 다른 mask backend를 연결하더라도 native mask 적용·원본 보존·job/Undo 계약은 공유할 수 있도록 설계한다. 지원 UI나 backend 배포 방식을 이 문서에서 미리 확정하지 않는다.

## 14. 구현 전 확정/검증할 결정 목록

아래는 설계 당시의 검토 목록이다. 최종 적용 범위와 타일링 탐구 종료 결정은16절을 따른다. 미검증 재질·환경은 품질 보증의 한계로 기록하며, 추가 탐구 작업으로 계속 진행하지 않는다.

1. 극단 비율의 지원 경계와 추가 model padding/안내 정책.
2. reference1MP·side4096·Fill4그룹·cutout16타일·halo/core/stride 초기값의 실제 적합성.
3. coarse 결과의 경계 외 미세구조를 놓치지 않는 정제 영역 계획과 상한 초과 처리.
4. Qwen tile refinement에서 전체 문맥/coarse guide를 전달하는 역할·프롬프트와 실제 효과.
5. RGBA cutout의 목표 opacity→relative mask 변환 및 원래 반투명 보존.
6. Comfy temp 사본의 실제 정리 수명과 사용할 API.
7. batch reroll·Poisson source의 기록 형태·메모리 상한.
8. 기존36MP보다 큰 native 문서 지원을 확장할 자원 기준.

이 결정은 구현 단계의 검증으로 해결하고 문서에 반영한다. 기술적 수치를 사용자에게 선택하게 하는 UI를 기본 해법으로 삼지 않는다. 사용자 승인이 필요한 것은 문서 크기의 의도적 변경 같은 명시적 동작이다.

## 15. 근거와 완료 보고 형식

공개 근거:

- [Qwen Image 2.1 공식 저장소](https://github.com/QwenLM/Qwen-Image-2.1): native2K 권장 크기, 다중 참조와 RGBA.
- [Qwen Image 2.1 Turbo 공식 카드](https://huggingface.co/Qwen/Qwen-Image-2.1-Turbo/blob/main/README.md): 권장 schedule과 동일한 해상도 표.
- [Comfy 공식 설명](https://comfy.org/qwen-image-2.1/): reference/RGBA 및 첫 입력 기준 latent.

현재 제품 경로는 [ComfyUI 연동 문서](comfyui.md)를 참조한다. 로컬 비교/역분석/원시 출력/빌드 기록은 gitignored `plan/2026-10-28-background-removal/`와 검토안 `plan/2026-10-28-qwen-edit-expansion-review/`에 별도 보관한다.

각 구현 단계 완료 보고에는 변경 파일·설계 결정·호출 수/크기·fake 테스트·실기 조건/결과·미검증 사항·회귀 여부를 포함한다. 단순 세션 생성과 실제 연산, 자동화 합격과 화질 합격, 원본 픽셀 보존과 완벽한 mask 추정을 각각 구분한다.

## 16. 확정된 구현 범위와 검증 결과

- **기본 기능:** 공통 native2K 면적/32padding 정책, 실제 RGBA 운반, 참조1장과 snapshot 수명, 사용자 승인 document resize·단일 Undo·실패 복구, 선택 영역 밖 보존, 독립 Remove 그룹의 원자적 단일 레이어, frozen reroll 및 영역별 수동 Poisson.
- **옵셔널 타일링 완료:** `edit.aiRemoveBackground`의 `experimentalRefine:true` 호출만 사용한다. 추가 UI 설정·가중치·custom node는 없다. alpha 초기 latent와 경계/부분 opacity/RGB detail work mask를 사용하고 scheduler의 마지막25% 구간에서 지정한 step 수로 정제한다. 타일의 작업 범위 밖은 native coarse float alpha를 복원하며, RGB는 항상 원본이다. guide는 padding을 포함한 전체 frame 좌표를 유지한다. 겹침의 반대 판정·coverage 누락·부분 오류는 전체 결과 적용을 막는다.
- **4090 실기:** 네이티브2400×1800 시료에서 엔진→Comfy→단일 mask 적용이52.86초에 성공했다. 같은 coarse 결과 대비 MAE1.332→1.177/255, IoU0.9850→0.9895, 가는 선 회수율93.74→95.77%, 작은 구멍 주변 MAE5.50→2.24였다. 단순512px 확대본 대신 원래 해상도에서 그린 구조로 검사했다.
- **12MP 비교:** 같은 원본/seed/Turbo4에서 기본 단일 경로21.03초·MAE0.719/255·IoU0.9885, 옵셔널 경로76.86초·MAE0.771/255·IoU0.9903이었다. 가는 선 회수율95.64→96.20%, 작은 구멍 주변 MAE4.75→2.69로 개선됐지만 평균 오차와 시간은 단일 경로가 유리했다. 이 결과를 일반적 품질 우열로 확대하지 않는다.
- **검증:** 최종 변경분 engine2588 tests, AI82/Comfy22 포함; engine/UI all-target Clippy, fmt, layers, wasm 및 adversarial panic hunt 통과. 앞선 UI4016 tests와 한국어 native/offscreen 참조·resize 승인/취소 시각 검증도 기록했다.
- **범위 종료:** 사용자 요청에 따라 타일 품질의 추가 탐구·새 생성 비교·자동 기본 경로 전환을 종료한다. 실제 머리카락·유리·연기·모든 모델/장치에서의 개선 보증은 하지 않는다. 단일 경로와 옵셔널 경로의 선택은 그대로 유지한다. Adobe 로컬 모델 연구/제품 연결은 별도 작업이다.

원시 근거는 local `plan/qwen-implementation/`의 gates JSON, `nvidia-native-detail`, `nvidia-pipeline`, `nvidia-12mp` 및 native UI 캡처에 있다. 테스트 이미지·원시 출력은 저장소에 배포하지 않는다.
