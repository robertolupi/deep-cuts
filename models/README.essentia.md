# Deep Cuts Models (Non-Commercial)

This repository hosts pre-trained classification models for the open-source [Deep Cuts](https://github.com/robertolupi/deep-cuts) desktop audio management application. 

These files are standard format-translated reproductions of the official pre-trained models released by the **Music Technology Group (MTG)** at **Universitat Pompeu Fabra**. Under copyright law, translating a file format (from TensorFlow to ONNX) without modifying the underlying model weights, parameters, or architecture is considered a format-translated reproduction and distribution, rather than the creation of a "derivative work."

---

## 📦 Model Inventory

| File | Purpose | Original License | Size |
|---|---|---|---|
| `genre_discogs400-discogs-effnet-1.onnx` | 400-class style/genre classifier | **CC BY-NC-ND 4.0** | ~2 MB |
| `genre_discogs400-discogs-effnet-1.json` | Style taxonomy label mappings | **CC BY-NC-ND 4.0** | ~12 KB |

---

## ⚖️ Licensing & Attribution (CC BY-NC-ND 4.0)

These models are distributed strictly under the **Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International (CC BY-NC-ND 4.0)** license.

### 🚫 Non-Commercial Notice
In accordance with the **NC** clause, these files **may not be used for commercial purposes** of any kind. 

### 🚫 No Derivatives / Format Translation Notice
These files are format conversions (from TensorFlow `.pb` to `.onnx` binary formats) designed solely to enable cross-platform native execution in the Svelte/Rust desktop application. No modifications, retraining, parameter updates, or alterations have been made to the underlying weights, neural architectures, or style taxonomy defined by the original creators.

### ✍️ Original Creator Attribution
Original pre-trained weights and style taxonomies are the property of the **Music Technology Group (MTG) / Universitat Pompeu Fabra**. 
* *Original Project:* [Essentia Pre-trained Models Catalog](https://essentia.upf.edu/models.html)
* *Citation:*
  > *Bogdanov, D., Wack, N., Gómez, E., Gulati, S., Herrera, P., Mayor, O., ... & Serra, X. (2013). Essentia: An audio analysis library for music information retrieval. In International Society for Music Information Retrieval Conference (ISMIR 2013).*
