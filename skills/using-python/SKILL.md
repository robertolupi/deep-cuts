---
name: using-python
description: How to run Python scripts and install packages in the deep-cuts project
---

# Using Python in Deep Cuts

Always use the project virtualenv — never the system Python or `python3` directly.

## Interpreter

```bash
tools/.venv/bin/python your_script.py
```

## Running a script

```bash
tools/.venv/bin/python tools/my_script.py
```

## Installing packages

```bash
tools/.venv/bin/pip install some-package
```

## Checking available packages

```bash
tools/.venv/bin/pip list
```

## Already installed

- `umap-learn`
- `numpy`
- `matplotlib`
- `scikit-learn`
