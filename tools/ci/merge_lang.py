import json
import subprocess
import sys


def stage(n, path):
    r = subprocess.run(["git", "show", f":{n}:{path}"], capture_output=True)
    if r.returncode != 0:
        return None
    return r.stdout.decode("utf-8")


def load(text):
    if text is None:
        return {}
    return json.loads(text)


def merge(base, ours, theirs):
    removed = {k for k in base if k not in ours}
    added = {k: v for k, v in ours.items() if k not in base}
    out = {}
    for k, v in theirs.items():
        if k in removed:
            continue
        if k in ours and k in base and ours[k] != base[k]:
            out[k] = ours[k]
        else:
            out[k] = v
    for k, v in added.items():
        out.setdefault(k, v)
    return out


def main(paths):
    for path in paths:
        ours_text = stage(2, path)
        ours = load(ours_text)
        nl = "\r\n" if ours_text and "\r\n" in ours_text else "\n"
        merged = merge(load(stage(1, path)), ours, load(stage(3, path)))
        text = json.dumps(merged, ensure_ascii=False, indent=2).replace("\n", nl) + nl
        with open(path, "w", encoding="utf-8", newline="") as f:
            f.write(text)


if __name__ == "__main__":
    main(sys.argv[1:])
