"""
MMLU Dataset and Parsing Utilities for Snarta Benchmark Suite.

Handles loading questions from JSON, CSV, or directories, mapping official 57 Hendrycks
subjects to high-level categories (STEM, Humanities, Social Sciences, Other),
downloading official benchmarks, and extracting multiple-choice predictions from LLM output.
"""

import csv
import json
import random
import re
import tarfile
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

BENCH_DIR = Path(__file__).resolve().parent
REPO_ROOT = BENCH_DIR.parent

# Mapping of Hendrycks MMLU 57 subjects and RuMMLU subjects to categories
MMLU_SUBJECT_CATEGORIES: Dict[str, str] = {
    # STEM
    "abstract_algebra": "STEM", "anatomy": "STEM", "astronomy": "STEM",
    "college_biology": "STEM", "college_chemistry": "STEM", "college_computer_science": "STEM",
    "college_mathematics": "STEM", "college_physics": "STEM", "computer_security": "STEM",
    "conceptual_physics": "STEM", "electrical_engineering": "STEM", "elementary_mathematics": "STEM",
    "high_school_biology": "STEM", "high_school_chemistry": "STEM", "high_school_computer_science": "STEM",
    "high_school_mathematics": "STEM", "high_school_physics": "STEM", "high_school_statistics": "STEM",
    "machine_learning": "STEM", "higher_mathematics": "STEM", "linear_algebra": "STEM",
    "differential_equations": "STEM", "probability_theory": "STEM", "complex_analysis": "STEM",
    "discrete_mathematics": "STEM", "theoretical_physics": "STEM", "quantum_mechanics": "STEM",
    "thermodynamics": "STEM", "electrodynamics": "STEM", "special_relativity": "STEM",
    "organic_chemistry": "STEM", "physical_chemistry": "STEM", "inorganic_chemistry": "STEM",
    "biochemistry": "STEM", "molecular_biology": "STEM", "genetics": "STEM",
    "human_physiology": "STEM", "algorithms": "STEM", "data_structures": "STEM",
    "computational_complexity": "STEM", "information_theory": "STEM", "computer_graphics": "STEM",
    "cryptography_math": "STEM",
    # Humanities
    "formal_logic": "Humanities", "high_school_european_history": "Humanities",
    "high_school_us_history": "Humanities", "high_school_world_history": "Humanities",
    "international_law": "Humanities", "jurisprudence": "Humanities",
    "logical_fallacies": "Humanities", "moral_disputes": "Humanities",
    "moral_scenarios": "Humanities", "philosophy": "Humanities", "prehistory": "Humanities",
    "professional_law": "Humanities", "world_religions": "Humanities", "ethics": "Humanities",
    "russian_history": "Humanities", "jurisprudence_ru": "Humanities",
    # Social Sciences
    "econometrics": "Social Sciences", "high_school_geography": "Social Sciences",
    "high_school_government_and_politics": "Social Sciences", "high_school_macroeconomics": "Social Sciences",
    "high_school_microeconomics": "Social Sciences", "high_school_psychology": "Social Sciences",
    "human_sexuality": "Social Sciences", "professional_psychology": "Social Sciences",
    "public_relations": "Social Sciences", "security_studies": "Social Sciences",
    "sociology": "Social Sciences", "us_foreign_policy": "Social Sciences",
    "microeconomics": "Social Sciences", "macroeconomics": "Social Sciences",
    "monetary_policy": "Social Sciences", "economic_thought": "Social Sciences",
    "game_theory": "Social Sciences", "cognitive_psychology": "Social Sciences",
    "social_psychology": "Social Sciences", "developmental_psychology": "Social Sciences",
    "political_science": "Social Sciences", "public_administration": "Social Sciences",
    "international_relations": "Social Sciences", "finance": "Social Sciences",
    "sociology_deviance": "Social Sciences", "macroeconomics_trade": "Social Sciences",
    "psychology_personality": "Social Sciences", "demography": "Social Sciences",
    # Russian Language & Culture
    "russian_orthography": "Russian Language & Culture", "russian_punctuation": "Russian Language & Culture",
    "russian_syntax": "Russian Language & Culture", "russian_morphology": "Russian Language & Culture",
    "russian_lexicology": "Russian Language & Culture", "russian_orthoepy": "Russian Language & Culture",
    "russian_paronyms": "Russian Language & Culture", "russian_phonetics": "Russian Language & Culture",
    "russian_linguistics": "Russian Language & Culture", "russian_literature": "Russian Language & Culture",
    "russian_poetics": "Russian Language & Culture", "russian_art": "Russian Language & Culture",
    "russian_music": "Russian Language & Culture", "russian_architecture": "Russian Language & Culture",
    "russian_theatre": "Russian Language & Culture",
    # Other
    "business_ethics": "Other", "clinical_knowledge": "Other", "college_medicine": "Other",
    "global_facts": "Other", "human_aging": "Other", "management": "Other",
    "marketing": "Other", "medical_genetics": "Other", "miscellaneous": "Other",
    "nutrition": "Other", "professional_accounting": "Other", "professional_medicine": "Other",
    "virology": "Other",
    # IT & Systems Edge Cases
    "it_edge_cases": "IT Edge Cases", "linux_kernel": "IT Edge Cases",
    "operating_systems": "IT Edge Cases", "networking": "IT Edge Cases",
    "databases": "IT Edge Cases", "concurrency": "IT Edge Cases",
    "concurrency_hardware": "IT Edge Cases", "memory_architecture": "IT Edge Cases",
    "rust_internals": "IT Edge Cases", "python_internals": "IT Edge Cases",
    "linux_signals": "IT Edge Cases", "compilers_linking": "IT Edge Cases",
    "cryptography_systems": "IT Edge Cases", "distributed_systems": "IT Edge Cases",
    "gpu_architecture": "IT Edge Cases", "storage_engines": "IT Edge Cases",
    "network_security": "IT Edge Cases",
}


@dataclass
class MmluQuestion:
    id: int
    subject: str
    category: str
    question: str
    choices: List[str]
    answer: str  # A, B, C, or D


@dataclass
class SingleQuestionEval:
    question_id: int
    subject: str
    category: str
    predicted: Optional[str]
    expected: str
    is_correct: bool
    latency_ms: float
    eval_tokens: int
    prompt_tokens: int
    response_text: str


@dataclass
class DatasetInfo:
    id: str
    name: str
    path: Path
    count: int
    categories: List[str]
    is_russian: bool = False
    is_hardcore: bool = False


DEFAULT_MMLU_CATEGORIES = ["All", "STEM", "Humanities", "Social Sciences", "Other", "IT Edge Cases"]
DEFAULT_RUMMLU_CATEGORIES = ["All", "STEM", "Humanities", "Social Sciences", "Russian Culture", "IT Edge Cases"]
DEFAULT_HARDCORE_CATEGORIES = [
    "All", "Math & Algo", "Cognitive Traps", "Low-Level Systems", "Formal Logic", "Adversarial Syntax"
]


CYRILLIC_CHOICE_MAP: Dict[str, str] = {
    "А": "A", "а": "A",
    "Б": "B", "б": "B",
    "В": "B", "в": "B",
    "С": "C", "с": "C",
    "Д": "D", "д": "D",
}
CHOICE_REGEX_LETTERS = r"[A-Da-d\u0410\u0430\u0411\u0431\u0412\u0432\u0421\u0441\u0414\u0434]"


def normalize_choice_letter(ch: str) -> str:
    """Normalizes ASCII and Cyrillic choice letters to standard A, B, C, D."""
    return CYRILLIC_CHOICE_MAP.get(ch, ch.upper())


def is_cyrillic(text: str) -> bool:
    """Checks whether the text contains Cyrillic characters."""
    return bool(re.search(r"[\u0400-\u04FF]", text))


def format_mmlu_prompt(q: MmluQuestion) -> str:
    """Formats an academic multiple-choice examination prompt in question's native language."""
    choices_str = "\n".join([f"{['A', 'B', 'C', 'D'][c_idx]}. {c}" for c_idx, c in enumerate(q.choices)])
    subj_display = q.subject.replace("_", " ")

    if is_cyrillic(q.question) or is_cyrillic(q.category):
        return (
            f"Вы сдаете академический экзамен с выбором одного правильного ответа.\n"
            f"Тема: {subj_display}.\n"
            f"Укажите ТОЛЬКО одну букву правильного варианта: A, B, C или D.\n\n"
            f"Вопрос: {q.question}\n{choices_str}\n\n"
            f"Ответ:"
        )
    return (
        f"You are taking an academic multiple-choice examination.\n"
        f"Answer the following question about {subj_display}.\n"
        f"Provide ONLY the single letter of the correct option: A, B, C, or D.\n\n"
        f"Question: {q.question}\n{choices_str}\n\n"
        f"Answer:"
    )


def extract_predicted_choice(text: str) -> Optional[str]:
    """Robustly extracts the multiple choice letter (A, B, C, D) from response."""
    if not text:
        return None

    # If thoughts are enclosed in <thought>...</thought> or <think>...</think>, prioritize text AFTER the thought
    post_thought = text
    for tag in ("thought", "think"):
        close_tag = f"</{tag}>"
        if close_tag in text.lower():
            idx = text.lower().rfind(close_tag)
            after = text[idx + len(close_tag):].strip()
            if after:
                post_thought = after
                break

    candidates = [post_thought, text]
    for target in candidates:
        cleaned = target.strip()
        if len(cleaned) == 1 and (cleaned.upper() in ("A", "B", "C", "D") or cleaned in CYRILLIC_CHOICE_MAP):
            return normalize_choice_letter(cleaned)

        # [A], (A), **A**
        m = re.search(rf"[\[\(]({CHOICE_REGEX_LETTERS})[\]\)]", target)
        if m:
            return normalize_choice_letter(m.group(1))

        # Russian / English prefix: Answer: A, Ответ: А, Вариант: B, Choice: C
        m = re.search(
            rf"(?:answer|choice|option|is|ответ|вариант|выбор)\s*(?:is|это|:|\.|\-)?\s*[\*\_]*({CHOICE_REGEX_LETTERS})\b",
            target,
            re.IGNORECASE,
        )
        if m:
            return normalize_choice_letter(m.group(1))

        # Final answer is X / Правильный ответ X
        m = re.search(rf"(?:final\s+answer|правильный\s+ответ).*?\b({CHOICE_REGEX_LETTERS})\b", target, re.IGNORECASE)
        if m:
            return normalize_choice_letter(m.group(1))

        # Word boundary
        m = re.search(rf"\b({CHOICE_REGEX_LETTERS})\b", target)
        if m:
            return normalize_choice_letter(m.group(1))

    return None


def normalize_answer(ans: Any, choices: List[str]) -> str:
    """Robustly normalizes answer labels from varied formats (letter, cyrillic, digit, choice text)."""
    if isinstance(ans, int):
        if 0 <= ans < len(choices):
            return chr(ord("A") + ans)
        if 1 <= ans <= len(choices):
            return chr(ord("A") + ans - 1)

    ans_str = str(ans).strip()
    if not ans_str:
        return "A"

    up = ans_str.upper()
    if up in ("A", "B", "C", "D", "E", "F"):
        return up

    # Cyrillic letters to standard Latin
    cyr = {"А": "A", "Б": "B", "В": "C", "Г": "D", "Д": "D"}
    if up in cyr:
        return cyr[up]

    # Patterns like (A), [A], A), A., Option A, Choice A
    m = re.match(r"^(?:option|choice)?\s*[\(\[]?([A-Da-dА-Га-г])[\)\]\.]?", ans_str, re.IGNORECASE)
    if m:
        letter = m.group(1).upper()
        return cyr.get(letter, letter)

    # Digit string ("0", "1", "2", "3")
    if ans_str.isdigit():
        val = int(ans_str)
        if 0 <= val < len(choices):
            return chr(ord("A") + val)
        if 1 <= val <= len(choices):
            return chr(ord("A") + val - 1)

    # Match answer string against choice text directly
    for idx, c in enumerate(choices):
        if str(c).strip().lower() == ans_str.lower():
            return chr(ord("A") + idx)

    return "A"


def robust_json_load(text: str) -> Any:
    """Loads JSON with tolerance for comments, trailing commas, and unescaped control chars."""
    try:
        return json.loads(text, strict=False)
    except Exception:
        # Strip // comments
        cleaned = re.sub(r"//.*?\n", "\n", text)
        # Strip trailing commas before } and ]
        cleaned = re.sub(r",\s*([}\]])", r"\1", cleaned)
        try:
            return json.loads(cleaned, strict=False)
        except Exception:
            # Fallback to JSON Lines (jsonl)
            items = []
            for line in text.splitlines():
                line = line.strip()
                if not line or line.startswith("//") or line.startswith("#"):
                    continue
                try:
                    items.append(json.loads(line, strict=False))
                except Exception:
                    pass
            if items:
                return items
            raise


def parse_dataset_file(file_path: Path) -> List[MmluQuestion]:
    """Parses questions from any JSON, JSONL, or CSV benchmark file."""
    questions: List[MmluQuestion] = []
    if not file_path.exists() or not file_path.is_file():
        return questions

    stem = file_path.stem
    default_sub = stem.replace("_sample", "").replace("_test", "")
    suffix = file_path.suffix.lower()

    if suffix in (".json", ".jsonl"):
        try:
            with open(file_path, "r", encoding="utf-8-sig") as f:
                raw_text = f.read()
            raw = robust_json_load(raw_text)
        except Exception:
            return []

        raw_items = []
        if isinstance(raw, list):
            raw_items = raw
        elif isinstance(raw, dict):
            for k in ("questions", "data", "tests", "items", "examples"):
                if k in raw and isinstance(raw[k], list):
                    raw_items = raw[k]
                    break
            if not raw_items and ("question" in raw or "prompt" in raw):
                raw_items = [raw]

        for idx, item in enumerate(raw_items):
            if not isinstance(item, dict):
                continue
            q_text = str(
                item.get("question")
                or item.get("prompt")
                or item.get("text")
                or item.get("instruction")
                or item.get("q")
                or ""
            ).strip()
            if not q_text:
                continue

            raw_choices = item.get("choices") or item.get("options") or item.get("variants") or item.get("answers")
            choices: List[str] = []
            if isinstance(raw_choices, dict):
                choices = [str(raw_choices[k]) for k in sorted(raw_choices.keys())]
            elif isinstance(raw_choices, (list, tuple)):
                choices = [str(c) for c in raw_choices]
            else:
                for letter in ("A", "B", "C", "D"):
                    if letter in item:
                        choices.append(str(item[letter]))

            # Clean leading A. / A) prefixes from choice text
            cleaned_choices = []
            for c in choices:
                c_clean = re.sub(r"^[A-Da-dА-Га-г][\.\)]\s+", "", c.strip())
                cleaned_choices.append(c_clean)

            ans_raw = item.get("answer") or item.get("correct_answer") or item.get("label") or "A"
            norm_ans = normalize_answer(ans_raw, cleaned_choices)

            sub = str(item.get("subject") or item.get("topic") or default_sub)
            cat = str(item.get("category") or item.get("cat") or "")
            if not cat:
                cat = MMLU_SUBJECT_CATEGORIES.get(sub.lower(), "")
            if not cat:
                low_q = (q_text + " " + sub).lower()
                if any(k in low_q for k in ("rust", "python", "linux", "kernel", "network", "socket", "database", "sql", "memory", "thread", "concurrency", "lock", "cpu", "gpu", "compiler", "cache", "async", "tokio")):
                    cat = "IT Edge Cases"
                elif any(k in low_q for k in ("math", "algebra", "integral", "matrix", "physics", "chemistry", "biology", "graph")):
                    cat = "STEM"
                elif any(k in low_q for k in ("history", "philosophy", "law", "logic", "syllogism", "ethics")):
                    cat = "Humanities"
                elif any(k in low_q for k in ("economy", "market", "sociology", "psychology", "politics")):
                    cat = "Social Sciences"
                elif is_cyrillic(q_text):
                    cat = "Russian Culture"
                else:
                    cat = "Other"

            q_id = item.get("id")
            try:
                final_id = int(q_id) if q_id is not None else idx + 1
            except Exception:
                final_id = idx + 1

            questions.append(MmluQuestion(
                id=final_id,
                subject=sub,
                category=cat,
                question=q_text,
                choices=cleaned_choices,
                answer=norm_ans,
            ))

    elif suffix == ".csv":
        with open(file_path, "r", encoding="utf-8-sig", newline="") as f:
            reader = csv.reader(f)
            first = True
            for row in reader:
                if not row:
                    continue
                if first and row[0].lower().strip() in ("question", "prompt", "q"):
                    first = False
                    continue
                first = False
                if len(row) >= 6:
                    raw_choices = [str(c).strip() for c in row[1:5]]
                    norm_ans = normalize_answer(row[5], raw_choices)
                    questions.append(MmluQuestion(
                        id=len(questions) + 1,
                        subject=default_sub,
                        category="Other",
                        question=str(row[0]).strip(),
                        choices=raw_choices,
                        answer=norm_ans,
                    ))

    return questions


def scan_available_datasets(search_dirs: Optional[List[Path]] = None) -> List[DatasetInfo]:
    """Scans benchmark data directories for all valid JSON and CSV datasets."""
    dirs = search_dirs or [
        BENCH_DIR / "data",
        REPO_ROOT / "bench" / "data",
        Path.cwd() / "bench" / "data",
        Path.cwd() / "data",
    ]
    seen_paths = set()
    datasets: List[DatasetInfo] = []

    def friendly_name(path: Path, count: int) -> str:
        stem = path.stem.lower()
        if "hardcore" in stem or "stress" in stem:
            return f"Hardcore Stress (Traps & Logic, {count}q)"
        if "rummlu" in stem or "mera" in stem:
            return f"RuMMLU / MERA (Russian, {count}q)"
        if "mmlu" in stem and "snarta" not in stem:
            return f"English MMLU ({count}q)"

        special = {
            "chatgpt": "ChatGPT",
            "deepseek": "DeepSeek",
            "it": "IT",
            "llm": "LLM",
            "mera": "MERA",
            "mmlu": "MMLU",
            "rummlu": "RuMMLU",
            "snarta": "Snarta",
            "gpu": "GPU",
            "rocm": "ROCm",
            "hip": "HIP",
            "test": "Test",
            "tests": "Tests",
        }
        clean_stem = path.stem.replace("-", "_")
        words = clean_stem.split("_")
        formatted = []
        for w in words:
            low = w.lower()
            if low in special:
                formatted.append(special[low])
            else:
                formatted.append(w.capitalize())
        title = " ".join(formatted)
        return f"{title} ({count}q)"

    for d in dirs:
        if not d.is_dir():
            continue
        for ext in ("*.json", "*.jsonl", "*.csv"):
            for f in sorted(d.glob(ext)):
                r = f.resolve()
                if r in seen_paths:
                    continue
                seen_paths.add(r)

                questions = parse_dataset_file(r)
                if not questions:
                    continue

                cnt = len(questions)
                is_ru = any(is_cyrillic(q.question) or is_cyrillic(q.category) for q in questions)
                raw_cats = set(q.category for q in questions)
                is_hard = any("Math & Algo" in c or "Cognitive Traps" in c or "Multi-Step" in c for c in raw_cats)

                stem = r.stem.lower()
                if "hardcore" in stem or "stress" in stem or is_hard:
                    cats = list(DEFAULT_HARDCORE_CATEGORIES)
                elif "rummlu" in stem or "mera" in stem:
                    cats = list(DEFAULT_RUMMLU_CATEGORIES)
                elif "mmlu" in stem and "snarta" not in stem:
                    cats = list(DEFAULT_MMLU_CATEGORIES)
                else:
                    sorted_cats = sorted(raw_cats)
                    if "Russian Language & Culture" in sorted_cats:
                        idx_c = sorted_cats.index("Russian Language & Culture")
                        sorted_cats[idx_c] = "Russian Culture"
                    cats = ["All"] + sorted_cats

                name = friendly_name(r, cnt)
                datasets.append(DatasetInfo(
                    id=r.stem,
                    name=name,
                    path=r,
                    count=cnt,
                    categories=cats,
                    is_russian=is_ru,
                    is_hardcore=is_hard,
                ))

    # Priority sorting: Hardcore, RuMMLU, MMLU, then custom datasets
    def sort_key(d: DatasetInfo) -> Tuple[int, str]:
        s = d.id.lower()
        if "hardcore" in s:
            return (0, d.name)
        if "rummlu" in s or "mera" in s:
            return (1, d.name)
        if "mmlu" in s and "snarta" not in s:
            return (2, d.name)
        return (3, d.name)

    datasets.sort(key=sort_key)
    return datasets


def load_dataset(
    dataset_path: Optional[str] = None,
    limit: Optional[int] = None,
    limit_per_subject: Optional[int] = None,
    categories_filter: Optional[List[str]] = None,
    subjects_filter: Optional[List[str]] = None,
    seed: int = 42,
) -> List[MmluQuestion]:
    """Loads questions from JSON, CSV, or directory with alias and path resolution."""
    path: Optional[Path] = None
    if dataset_path:
        alias = str(dataset_path).lower().strip()
        cand = Path(dataset_path)
        if cand.exists():
            path = cand.resolve()
        elif (BENCH_DIR / "data" / cand.name).exists():
            path = (BENCH_DIR / "data" / cand.name).resolve()
        elif (BENCH_DIR / "data" / f"{cand.stem}.json").exists():
            path = (BENCH_DIR / "data" / f"{cand.stem}.json").resolve()
        elif (REPO_ROOT / cand).exists():
            path = (REPO_ROOT / cand).resolve()
        elif alias in ("hardcore", "stress", "hardcore_stress", "hard"):
            path = (BENCH_DIR / "data" / "hardcore_stress_sample.json").resolve()
        elif alias in ("rummlu", "mera", "ru", "russian"):
            path = (BENCH_DIR / "data" / "rummlu_sample.json").resolve()
        elif alias in ("mmlu", "en", "english"):
            path = (BENCH_DIR / "data" / "mmlu_sample.json").resolve()
        elif alias in ("chatgpt", "it_chatgpt", "snarta_it_test_chatgpt"):
            path = (BENCH_DIR / "data" / "snarta_it_test_chatgpt.json").resolve()
        elif alias in ("deepseek", "it_deepseek", "snarta_it_test_deepseek"):
            path = (BENCH_DIR / "data" / "snarta_it_test_deepseek.json").resolve()
        else:
            for ds in scan_available_datasets():
                if ds.id.lower() == alias or ds.path.stem.lower() == alias:
                    path = ds.path.resolve()
                    break
    else:
        path = (BENCH_DIR / "data" / "mmlu_sample.json").resolve()

    if not path or not path.exists():
        raise FileNotFoundError(
            f"Dataset not found for '{dataset_path}'. Available in bench/data: "
            f"{[d.name for d in scan_available_datasets()]}"
        )

    if path.is_dir():
        questions: List[MmluQuestion] = []
        for csv_file in sorted(path.glob("**/*_test.csv")):
            sub = csv_file.stem.replace("_test", "")
            cat = MMLU_SUBJECT_CATEGORIES.get(sub, "Other")
            with open(csv_file, "r", encoding="utf-8-sig", newline="") as f:
                reader = csv.reader(f)
                for row in reader:
                    if len(row) >= 6:
                        raw_choices = [str(c).strip() for c in row[1:5]]
                        norm_ans = normalize_answer(row[5], raw_choices)
                        questions.append(MmluQuestion(
                            id=len(questions) + 1,
                            subject=sub,
                            category=cat,
                            question=row[0].strip(),
                            choices=raw_choices,
                            answer=norm_ans,
                        ))
    else:
        questions = parse_dataset_file(path)

    # Apply filters
    if categories_filter:
        norm_cats = [c.strip().lower() for c in categories_filter]
        if "russian culture" in norm_cats:
            norm_cats.append("russian language & culture")
        questions = [q for q in questions if q.category.lower() in norm_cats]

    if subjects_filter:
        norm_subs = [s.strip().lower() for s in subjects_filter]
        questions = [q for q in questions if q.subject.lower() in norm_subs]

    if limit_per_subject and limit_per_subject > 0:
        by_sub: Dict[str, List[MmluQuestion]] = {}
        for q in questions:
            by_sub.setdefault(q.subject, []).append(q)
        trimmed: List[MmluQuestion] = []
        for sub_qs in by_sub.values():
            trimmed.extend(sub_qs[:limit_per_subject])
        questions = trimmed

    if limit and limit > 0 and len(questions) > limit:
        rng = random.Random(seed)
        questions = rng.sample(questions, limit)

    return questions


def download_official_mmlu(dest_dir: Path) -> Path:
    """Downloads Hendrycks MMLU data.tar and extracts test split."""
    url = "https://people.eecs.berkeley.edu/~hendrycks/data.tar"
    dest_dir.mkdir(parents=True, exist_ok=True)
    tar_path = dest_dir / "data.tar"
    print(f"Downloading official MMLU dataset from {url}...")
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=120) as resp, open(tar_path, "wb") as out_file:
        total = int(resp.headers.get("Content-Length", 0))
        downloaded = 0
        while True:
            chunk = resp.read(1024 * 512)
            if not chunk:
                break
            out_file.write(chunk)
            downloaded += len(chunk)
            if total > 0:
                pct = (downloaded / total) * 100.0
                print(f"\r  Progress: {downloaded / 1024 / 1024:.1f}/{total / 1024 / 1024:.1f} MB ({pct:.1f}%)", end="", flush=True)
    print("\nExtracting test split...")
    with tarfile.open(tar_path, "r") as tar:
        for member in tar.getmembers():
            if "test/" in member.name and member.name.endswith(".csv"):
                tar.extract(member, path=dest_dir)
    print("Download and extraction complete.")
    return dest_dir / "data" / "test"


def find_matching_mtp_layer(model_path: Path) -> Optional[Path]:
    """Finds companion mtp-*.gguf speculative draft layer sitting near the model."""
    search_dirs: List[Path] = []
    if model_path.parent.is_dir():
        search_dirs.append(model_path.parent)
    search_dirs.extend([
        Path("models"),
        Path("target/release/models"),
    ])

    model_lower = model_path.name.lower()
    is_gemma4 = "gemma-4" in model_lower or "gemma4" in model_lower

    for d in search_dirs:
        if not d.exists() or not d.is_dir():
            continue
        for f in d.glob("*.gguf"):
            fname = f.name.lower()
            if "mtp" in fname:
                if is_gemma4 and ("gemma-4" in fname or "gemma4" in fname or fname.startswith("mtp-")):
                    return f.resolve()
                elif not is_gemma4 and "gemma" not in fname:
                    return f.resolve()
    return None
