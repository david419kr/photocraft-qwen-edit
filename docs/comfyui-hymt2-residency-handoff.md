# PhotoCraft + ComfyUI + Qwen2.1 동일 구성: Hy-MT2 VRAM 상주 패치 인수인계

작성일: 2026-10-12

대상: 이미 PhotoCraft의 로컬 번역 및 Qwen Image 2.1 생성/편집이 작동하는 다른 PC 또는 다른 portable 설치본.

## 배포 업데이트: 2026-10-12

상주 패치를 사용자 fork에 커밋·푸시했다: [22ea3d7](https://github.com/david419kr/ComfyUI-Hy-MT2/commit/22ea3d7ebc8938515c0f60dbcd991edbb46ae8d5).
PhotoCraft 설치 스크립트는 이 커밋을 고정해서 설치한다. 전체 `/free` 처리 시 공유 번역 워커뿐 아니라 이전 이미지 로더 기억도 초기화한다.
아래 코드와 실측은 최초 로컬 패치의 인계 기록이며, 최신 배포 소스는 링크한 커밋을 기준으로 한다.
PhotoCraft의 후속 변경은 모델 선택 변경 시 전체 `/free` 요청과 큐 확인 작업을 제출하며, 실제 실행 모델 변경도 추적한다. 같은 모델의 번역·생성 반복은 해제하지 않는다.

이 문서는 현재 PC에 **실제로 설치한 변경사항**을 같은 구성에 이식하기 위한 것입니다. 아래 신규 파일 전체와 교체 메서드를 포함하므로 MD 하나로 적용할 수 있습니다.

## 1. 적용 목표와 전제

- 번역 → 이미지 생성 → 번역으로 작업이 바뀌어도, 메모리 여유가 있으면 번역 모델과 이미지 모델을 재사용한다.
- VRAM 부족 시에만 필요한 공간을 확보한다. 명시적 모델 언로드, 번역 모델 교체, 종료 요청은 계속 처리한다.
- 기존 번역 문구, 스타일, 샘플링 설정, 모델 파일, PhotoCraft 기능은 유지한다.
- PhotoCraft 바이너리와 ComfyUI 코어 소스는 이번 패치의 수정 대상이 아니다.

대상 환경에서 다음을 먼저 확인한다.

1. `ComfyUI/custom_nodes/ComfyUI-Hy-MT2`에 `HyMT_Translation_GGUF`가 있다.
2. `py/isolated_llama.py`와 `runtime/llama_worker.py`가 있고, 노드의 `.venv` Python으로 별도 워커 프로세스를 구동한다.
3. 현재 번역과 이미지 생성이 정상 작동한다.
4. 이미지 그래프가 독립적인 `UNETLoader`, `CLIPLoader`, `VAELoader`를 사용한다. PhotoCraft의 현행 그래프가 이 구성이다.
5. `comfy.model_management.free_memory`, `get_free_memory`, `get_torch_device`, `minimum_inference_memory` 및 `PromptServer.instance.add_on_prompt_handler`가 존재한다.

일반 upstream Hy-MT2처럼 ComfyUI Python에서 llama-cpp-python을 직접 import하는 구성이라면 먼저 전용 워커 구성이 필요하다. fork 설치 안내: https://github.com/david419kr/ComfyUI-Hy-MT2 . 상주 패치를 포함하는 배포 커밋은 위 업데이트를 참조한다.

## 2. 원인: 실제 코드와 로그에서 확인한 내용

### 2.1 번역 모델

기존 번역 노드는 인스턴스의 `self.llm`에 워커 프록시를 보관했다. `unload_after_run=false`여도 ComfyUI가 그래프 전환으로 노드 인스턴스를 캐시에서 제거하면 프록시 소멸자의 `close()`가 워커를 종료할 수 있었다. 따라서 다음 번역에서 다시 로딩됐다.

### 2.2 이미지 모델

PhotoCraft는 번역과 이미지 생성을 별도 ComfyUI 작업으로 제출한다. 확인한 ComfyUI 0.37.4 기본 RAM-pressure 캐시는 비활성 캐시의 RAM headroom을 시스템 RAM 전체(최대 128GB)로 잡는다. 확인 PC는 약 64GB RAM이므로 이 여유를 만족하지 못했고, 번역 작업 중 이전 이미지 로더 출력이 비활성 캐시로 제거됐다. 모델 참조가 사라지면서 VRAM이 남아 있어도 다음 이미지 작업에서 파일을 다시 읽는 패턴이 발생했다.

이번 패치는 RAM 캐시의 전역 기준을 변경하는 대신, 번역 작업에도 이전 이미지 로더의 캐시 키를 유지하도록 한다.

## 3. 설치 노드에 적용한 구체적 변경 목록

| 파일 | 변경 내용 |
|---|---|
| `py/residency.py` | 신규. 공유 번역 워커, 사용 중 보호, VRAM 부족 시 해제, 번역 시 이미지 로더 캐시 유지, ComfyUI 연결 훅 |
| `py/hy_mt_gguf_node.py` | 인스턴스별 워커 소유 제거. 공유 워커 사용. 번역 전체를 사용 중 세션으로 감싸고 `unload_after_run=true`를 `finally`에서 처리 |
| `py/isolated_llama.py` | `Llama.is_alive()` 추가. 기존 프로세스가 살아 있는지 확인 |
| `__init__.py` | 노드 로딩 뒤 `install_residency()` 호출 추가 |

배포 과정에서 `__init__.py`의 표시 버전도 1.0.0 → 1.0.1로 바뀌었다. 이 표시 변경은 상주 동작에 필요한 요소가 아니다.

### 변경 후 실제 동작 규칙

- 공유 워커는 설치 노드 모듈 안에 하나만 유지한다. 번역 모델 두 종류를 동시에 올리는 모델 풀은 아니다.
- 캐시 서명은 `(절대 모델 경로, gpu_layers)`이다. 같고 프로세스가 살아 있으면 재사용한다. 다르면 기존 워커를 닫고 교체한다.
- 번역 전체를 `RLock`과 `busy` 플래그로 보호한다. 메모리 압력 훅은 사용 중 워커를 강제로 닫지 않는다.
- 새 CUDA 번역 모델 로딩 예산은 **GGUF 파일 크기 + 1GiB**다. KV/work buffer를 포함한 간단한 추정이며 정확한 실측치가 아니다.
- `get_free_memory(device)`가 예산보다 작을 때만 `free_memory(예산, device)`를 호출한다. 무조건 `unload_all_models()`로 전체를 비우지 않는다.
- 외부 CUDA 워커는 PyTorch의 미사용 예약 페이지를 직접 쓸 수 없다. 물리적 free가 예산보다 작으면 `soft_empty_cache()`로 미사용 캐시를 돌려준다. 살아 있는 이미지 가중치를 모두 지우는 동작과는 다르다.
- 기존 `free_memory(memory_required, device, ...)`를 감싸되 인수와 반환값은 유지한다.
- 일반 요청에서는 `free < memory_required`일 때 해당 장치의 idle 번역 워커를 해제한다.
- DynamicVRAM 요청에서는 `free < min(memory_required, minimum_inference_memory())`로 판정한다. 가상 가중치 크기만으로 무조건 해제하지 않기 위한 처리다.
- `/free`의 `unload_models=true`는 ComfyUI의 큰 메모리 해제 요청을 통해 번역 워커도 닫는다. `free_memory=true`를 함께 주면 그래프 캐시도 초기화한다.
- `atexit`으로 정상 ComfyUI 종료 시 공유 워커를 닫는다.
- 이미지 로더 캐시 보존은 `HyMT_Translation_GGUF`와 `PreviewAny`만 있는 번역 그래프에 한정한다.
- 이전 그래프에서 독립 로더 3종의 노드 ID·입력을 기억하고, 번역 그래프에 연결되지 않은 노드로 넣는다. 캐시 키를 유지할 뿐 이미지 추론을 추가 실행하지 않는다.
- 연결 입력(list/dict)이 있는 로더는 제외한다. 존재하지 않는 의존 노드를 복제하지 않는다. `setdefault()`로 기존 노드 ID를 덮어쓰지 않는다.
- hook의 `_hymt_residency` 표식으로 동일 프로세스 내 중복 설치를 방지한다.

## 4. 대상 설치본에 적용하는 순서

1. 대상 portable 루트 및 실제 사용 중인 ComfyUI 프로세스 경로를 확인한다. 이 문서의 D: 경로를 다른 PC에 하드코딩하지 않는다.
2. ComfyUI를 정상 종료한다.
3. 변경할 기존 파일을 `custom_nodes` **밖**에 백업한다. 백업 폴더를 노드 폴더 옆에 두어 중복 로딩시키지 않는다.
4. 아래 4개 파일 변경을 적용한다. `.venv`, `.python`, 모델 파일, 기존 CUDA DLL 로딩 방식은 유지한다.
5. 기존 방식으로 ComfyUI를 다시 실행한다. 기존 포트와 실행 옵션을 유지한다.
6. §6의 최소 확인을 한다. PhotoCraft를 다시 빌드하거나 모델을 다시 다운로드할 필요는 없다.

### 4.1 신규 `py/residency.py`: 파일 전체

```python
# SPDX-License-Identifier: MIT
"""Keep the private translator across workflows, with ComfyUI pressure eviction."""
import atexit
import copy
import functools
import logging
import threading
from contextlib import contextmanager
from pathlib import Path

from .isolated_llama import Llama

LOG = logging.getLogger(__name__)
GIB = 1024 ** 3


class ResidentTranslator:
    def __init__(self):
        self.lock = threading.RLock()
        self.llm = None
        self.signature = None
        self.device = None
        self.busy = False

    @contextmanager
    def session(self):
        with self.lock:
            self.busy = True
            try:
                yield
            finally:
                self.busy = False

    def acquire(self, model_path, gpu_layers=-1):
        signature = (str(Path(model_path).resolve()), int(gpu_layers))
        if self.signature == signature and self.llm is not None and self.llm.is_alive():
            LOG.info("[HyMT residency] Reusing translation worker")
            return self.llm
        self.release("model change or worker exit")
        if gpu_layers != 0:
            import comfy.model_management as mm
            self.device = mm.get_torch_device()
            # Include KV/work buffers; release only the deficit, never all image models.
            if self.device.type == "cuda":
                required = Path(model_path).stat().st_size + GIB
                if mm.get_free_memory(self.device) < required:
                    LOG.info("[HyMT residency] Making room for translation: %.0f MB", required / 1024 ** 2)
                    mm.free_memory(required, self.device)
                # An external CUDA process cannot use PyTorch's unused reserved pages.
                if mm.torch.cuda.mem_get_info(self.device)[0] < required:
                    mm.soft_empty_cache()
        llm = Llama(model_path=str(model_path), n_gpu_layers=int(gpu_layers), n_ctx=8192, verbose=False)
        self.llm = llm
        self.signature = signature
        LOG.info("[HyMT residency] Translation worker loaded")
        return llm

    def release(self, reason):
        with self.lock:
            if self.llm is not None:
                self.llm.close()
                LOG.info("[HyMT residency] Translation worker unloaded: %s", reason)
            self.llm = None
            self.signature = None
            self.device = None

    def release_for_pressure(self, required, device, mm, dynamic=False):
        if not self.lock.acquire(blocking=False):
            return
        try:
            if self.llm is None or self.busy or device != self.device:
                return
            # DynamicVRAM requests virtual model sizes; evict only for real headroom loss.
            target = min(required, mm.minimum_inference_memory()) if dynamic else required
            if mm.get_free_memory(device) < target:
                self.release("VRAM pressure or explicit unload")
        finally:
            self.lock.release()


TRANSLATOR = ResidentTranslator()
atexit.register(TRANSLATOR.release, "ComfyUI exit")


class ImageLoaderKeepalive:
    """Unused loader nodes retain cache keys without executing image inference."""
    def __init__(self):
        self.loaders = {}

    def __call__(self, data):
        prompt = data.get("prompt", {})
        if not isinstance(prompt, dict):
            return data
        kinds = {node.get("class_type") for node in prompt.values() if isinstance(node, dict)}
        if "HyMT_Translation_GGUF" in kinds and kinds <= {"HyMT_Translation_GGUF", "PreviewAny"}:
            for key, node in self.loaders.items():
                prompt.setdefault(key, copy.deepcopy(node))
            if self.loaders:
                LOG.info("[HyMT residency] Preserving %d image loader cache keys during translation", len(self.loaders))
        else:
            loaders = {
                key: copy.deepcopy(node) for key, node in prompt.items()
                if isinstance(node, dict) and node.get("class_type") in {"UNETLoader", "CLIPLoader", "VAELoader"}
                and isinstance(node.get("inputs"), dict)
                and all(not isinstance(value, (list, dict)) for value in node["inputs"].values())
            }
            if loaders:
                self.loaders = loaders
        return data


def install():
    import comfy.model_management as mm
    from server import PromptServer

    if getattr(mm.free_memory, "_hymt_residency", False):
        return
    original = mm.free_memory

    @functools.wraps(original)
    def free_memory(memory_required, device, *args, **kwargs):
        dynamic = kwargs.get("for_dynamic", args[1] if len(args) > 1 else False)
        TRANSLATOR.release_for_pressure(memory_required, device, mm, dynamic)
        return original(memory_required, device, *args, **kwargs)

    free_memory._hymt_residency = True
    mm.free_memory = free_memory
    PromptServer.instance.add_on_prompt_handler(ImageLoaderKeepalive())
    LOG.info("[HyMT residency] Persistent translation and image loader keepalive enabled")
```

### 4.2 `py/isolated_llama.py`: `Llama` 클래스에 추가

기존 `create_chat_completion()`과 `close()` 사이 등 클래스 내부에 다음 메서드만 추가한다.

```python
    def is_alive(self):
        process = getattr(self, "_process", None)
        return process is not None and process.poll() is None
```

이미 존재하면 중복 추가하지 않는다. 기존 환경변수 정리, Python 경로, DLL 탐색, 워커 프로토콜, timeout은 이번 이식에서 변경하지 않는다.

### 4.3 `py/hy_mt_gguf_node.py`: 인스턴스 소유 제거 및 메서드 교체

`HyMT_Translation_GGUF.__init__()`가 아래 두 필드만 설정한다면 해당 생성자를 제거한다. 대상 설치본에 다른 초기화가 있다면 그 초기화는 남기고 두 필드만 제거한다.

```python
self.llm = None
self.current_signature = None
```

`INPUT_TYPES`, `RETURN_TYPES`, `FUNCTION`, 모델 스캔/해석 함수, `NODE_CLASS_MAPPINGS`는 유지한다. `load_model()`, `unload_model()`, `translate()`를 아래로 교체하고 `_translate()`를 추가한다. 아래 번역 본문은 이번 설치본에서 사용한 그대로다. 대상에 의도적인 기존 번역 문구 변경이 있다면 해당 본문을 유지하면서 호출 대상만 `self.llm` → `llm`으로 바꾸고 동일한 세션 구조를 적용한다.

```python
    def load_model(self, model: str, gpu_layers: int = -1):
        from .residency import TRANSLATOR
        return TRANSLATOR.acquire(ensure_gguf_file(model), gpu_layers)

    def unload_model(self):
        from .residency import TRANSLATOR
        TRANSLATOR.release("unload_after_run")
        free_memory()

    def translate(
        self,
        model: str,
        translate_style: str,
        translate_to: str,
        text: str,
        unload_after_run: bool = False,
    ):
        if not text.strip():
            return ("",)

        from .residency import TRANSLATOR
        with TRANSLATOR.session():
            llm = self.load_model(model, gpu_layers=-1)
            try:
                return self._translate(llm, translate_style, translate_to, text)
            finally:
                if unload_after_run:
                    self.unload_model()

    def _translate(self, llm, translate_style, translate_to, text):
        masked_text, token_map = mask_prompt_tags(text.strip())
        target_lang = resolve_target_language(text, translate_to)
        chunks = split_text_into_chunks(masked_text, max_chars=3000)
        pbar = ProgressBar(len(chunks))

        translated_chunks = []
        for i, chunk in enumerate(chunks):
            pbar.update_absolute(i, len(chunks))
            style_info = STYLE_PROMPTS.get(translate_style)
            if style_info:
                if target_lang in ["Chinese", "Traditional Chinese", "Cantonese"]:
                    style_desc = style_info["zh"]
                    prompt = f"请将以下文本翻译为 {target_lang}。注意翻译的风格要严格符合【{style_desc}】，只需要输出翻译后的结果，不要额外解释：\n\n{chunk}"
                else:
                    style_desc = style_info["en"]
                    prompt = f"Please translate the following text into {target_lang}. Note that the translation style must strictly conform to [{style_desc}] and only output the translated result without any additional explanation:\n\n{chunk}"
            else:
                prompt = f"Translate the following text into {target_lang}. Note that you should only output the translated result without any additional explanation:\n\n{chunk}"
            messages = [{"role": "user", "content": prompt}]

            result = llm.create_chat_completion(
                messages=messages,
                max_tokens=4096,
                temperature=0.7,
                top_p=0.6,
                top_k=20,
                repeat_penalty=1.05,
                stop=["<|hy_end_of_assistant|>", "<|hy_begin_of_sentence|>", "<|im_end|>", "<|endoftext|>"],
            )

            choices = result.get("choices", [])
            raw_output = choices[0]["message"]["content"] if choices else ""
            clean_out = clean_model_output(raw_output)
            translated_chunks.append(clean_out)

        pbar.update_absolute(len(chunks), len(chunks))
        final_text = "\n\n".join(translated_chunks) if len(chunks) > 1 else (translated_chunks[0] if translated_chunks else "")
        if token_map:
            final_text = unmask_prompt_tags(final_text, token_map)

        return (final_text,)
```

### 4.4 `__init__.py`: 노드 로딩 뒤 초기화

기존 `load_nodes()` 호출 **직후** 다음 블록을 추가한다. 기존 `NODE_CLASS_MAPPINGS`와 `__all__`는 그대로 둔다.

```python
try:
    from .py.residency import install as install_residency
    install_residency()
except (ImportError, AttributeError) as error:
    print(f"[{__repo_name__}] Residency integration unavailable: {error}")
```

정상 시작 로그:

```text
[HyMT residency] Persistent translation and image loader keepalive enabled
```

`Residency integration unavailable`가 나오면 두 훅이 모두 정상 설치됐다고 간주하지 말고 대상 버전/초기화 시점을 확인한다.

## 5. 유지해야 하는 기존 설정 및 설치본 차이

### PhotoCraft 측 설정

- 번역 노드: `HyMT_Translation_GGUF`.
- 출력: `PreviewAny`; 현재 PhotoCraft 노드 ID는 번역 `901`, 출력 `902`다.
- `translate_to=English`, `translate_style=Default`, `unload_after_run=false`.
- 이미지 모델·텍스트 인코더·CFG·스텝 수와 기존 프롬프트 성형은 그대로 둔다.
- 동일 PhotoCraft 빌드라면 변경 코드는 ComfyUI 노드 안에만 필요하다.

### 전용 LLM 환경과 이번 패치의 관계

기존 설치본은 노드 하위 `.venv`의 Python으로 `runtime/llama_worker.py`를 띄우고 UTF-8 JSON을 stdin/stdout으로 주고받는다. 워커가 자기 환경의 llama-cpp-python을 import하며 ComfyUI Python의 패키지를 교체하지 않는다. 설치 PC의 llama-cpp-python 패키지는 `0.4.2+cu128`이었다.

기존 워커에는 `Hy-MT2-1.8B-heretic.i1-Q4_K_M.gguf`에 한해 EOS ID를 `120020`으로 보정하는 처리도 이미 있었다. 이것은 **이번 상주 패치에서 새로 추가한 변경이 아니다**.

중요한 실제 차이: 이 PC의 설치된 `runtime/llama_worker.py`와 `py/isolated_llama.py`는 로컬 fork의 최신 설치 환경 코드와 완전히 같지 않다. 설치본은 기존 CUDA 경로 로딩을 유지했고, fork 최신 버전은 private NVIDIA 패키지 DLL 경로 관리와 더 많은 환경변수 정리를 포함한다. 이번에는 기존 실행 환경을 업그레이드하지 않고 **상주 관련 부분만** 이식했다. 대상에서도 위 4개 변경을 위해 워커 전체나 `.venv`를 무조건 덮어쓰지 않는다.

## 6. 최소 확인 절차와 실제 확인 결과

추가 GPU 작업을 병렬 실행하지 않는다. 기존 모델로 작은 이미지 2회 정도만 순서대로 확인한다.

1. 서로 다른 짧은 한국어 프롬프트로 `번역 → 이미지 생성 → 번역 → 이미지 생성 → 번역`을 실행한다. 같은 번역 결과 캐시에만 맞는 테스트를 피한다.
2. 첫 로딩 이후 `[HyMT residency] Reusing translation worker`를 확인한다.
3. 번역 작업에서 `Preserving 3 image loader cache keys during translation`을 확인한다.
4. 두 번째 이미지 작업의 history `execution_cached`에 이전 UNET/CLIP/VAE 노드 ID가 포함되는지 확인한다. 현행 PhotoCraft는 `37`, `38`, `39`다.
5. 매번 safetensors를 읽는 `Model storage policy ... paths=[...]` 로그가 반복되는지 비교한다. `prepared for dynamic VRAM loading` 로그 자체만으로 파일 재로딩이라고 단정하지 않는다.
6. 여유 VRAM이 충분한 상황에서 번역 전후 모델 메모리가 전부 비워지지 않는지 확인한다.
7. 테스트가 끝나면 해당 ComfyUI에 다음 요청을 보내 워커 종료와 모델 해제를 확인한다. 이는 실제로 모델을 내리는 검증이다.

```http
POST /free
Content-Type: application/json

{"unload_models": true, "free_memory": true}
```

검증 로그 예:

```text
[HyMT residency] Reusing translation worker
[HyMT residency] Preserving 3 image loader cache keys during translation
[HyMT residency] Translation worker unloaded: VRAM pressure or explicit unload
```

이 PC에서 확인한 결과(2026-10-12):

| 항목 | 결과 |
|---|---|
| 환경 | Windows, RTX 4090 24GB, ComfyUI 0.37.4, DynamicVRAM |
| 모델 | Hy-MT2 1.8B Heretic Q4 + Qwen2.1 Turbo INT8 ConvRot + 순정 INT8 TE/VAE |
| 이미지 크기/스텝 | 512×512, 8스텝 |
| 번역 워커 | 최초 1회 로딩 후 이미지 생성 뒤에도 재사용 |
| 이미지 로더 | 두 번째 작업에서 37/38/39 모두 캐시 재사용 |
| 이미지 실행 시간 | 서버 로그 기준 최초 약 7.12초 → 다음 약 0.90초 |
| 후속 번역 | 서버 로그에서 약 0.06초 확인 |
| 여유 VRAM | 이미지 생성 뒤 약 7.96GiB, 번역 작업 전환 뒤에도 유지 |
| 명시적 언로드 | 워커 종료 및 이미지 해제 확인, 여유 VRAM 약 22.39GiB |
| 회귀 테스트 | 10개 통과: 재사용/교체, idle/다른 장치/사용 중 보호, DynamicVRAM 판정, 유한 로딩 예산, 명시적 언로드, 훅 중복/keyword 인수, 로더 보존/의존 입력 제외 |

물리 VRAM을 일부러 고갈시키는 테스트는 하지 않았다. 부족 시 분기와 사용 중 보호는 mock 기반 회귀 테스트로 확인했다. 위 시간은 해당 PC의 작은 이미지 측정값이며 다른 PC 성능 보장이 아니다.

## 7. 적용 범위와 한계

- 이 패치는 전용 프로세스 번역 워커 + 독립 로더 3종 + 번역 전용 그래프라는 현재 구성에 맞춘 것이다.
- 모델 파일 크기 + 1GiB는 예산 추정이다. 다른 크기 모델/긴 컨텍스트의 실제 KV 메모리까지 정확하게 측정하는 OOM 복구 엔진은 아니다.
- 이미지 가중치를 무조건 GPU에 고정하지 않는다. ComfyUI의 VRAM 관리와 실제 RAM 압력에 의한 해제는 계속 가능하다.
- 대상 ComfyUI가 훅 함수의 인수/수명주기 또는 캐시 키 정책을 바꿨다면 그 구현을 먼저 확인한다.
- 이미 다른 확장이 `free_memory`를 감쌌다면 원래 callable로 위임하는 순서를 유지한다. 이번 hook을 같은 프로세스에서 여러 번 중첩 설치하지 않는다.
- 다른 형태의 번역 그래프, 연결 입력을 가진 커스텀 로더, 여러 클라이언트가 섞여 큐를 쓰는 경우까지 별도 검증한 것은 아니다.
- 기존 소스의 라이선스는 유지한다. 신규 `py/residency.py`에는 MIT SPDX 헤더가 있다.

## 8. 근거 파일과 운영상 분리 사항

이 PC의 실제 설치 위치:

```text
D:\ai_drawing\ComfyUI_windows_portable\ComfyUI\custom_nodes\ComfyUI-Hy-MT2
```

PhotoCraft 쪽 로컬 증거(다른 PC에는 없을 수 있음):

```text
log/hymt-residency/before/                 수정 전 파일 백업
log/hymt-residency/runtime-results.json   실제 작업별 출력/캐시/메모리
log/hymt-residency/server-final.err.log   실제 ComfyUI 로그
log/hymt-residency/result.png             생성 결과
```

이 PC의 로컬 fork에는 `tests/test_residency.py`와 `tests/test_isolated_runtime.py`가 있다. 해당 테스트 파일을 함께 가진 환경에서는 `python -m unittest discover -s tests -p 'test_*.py' -v`로 실행한다.

신규 설치 파일 `py/residency.py` SHA-256:

```text
d526d09edd46b5320ad4f55adf04f81e72fdae4bf207fbdc54391b0c0700ca37
```

최근에 이 PC의 Windows WinNAT가 8188을 자동 예약한 문제는 OS 설정으로 따로 해제했다. **VRAM 패치에는 포트 예약 해제, 서비스 재시작, 포트 변경이 포함되지 않는다.** 다른 PC에서 같은 증상이 실제로 없다면 네트워크 설정을 건드릴 필요가 없다.

## 9. 대상 에이전트가 완료 후 보고할 항목

- 확인한 실제 portable/노드 경로와 ComfyUI 버전.
- 변경한 네 파일 및 백업 위치.
- 기존 전용 환경/프롬프트 설정이 보존됐는지.
- 번역 워커 재사용, 이미지 로더 캐시 재사용, 명시적 언로드 확인 결과.
- 수행하지 않은 검증과 대상 버전에 따른 차이.
