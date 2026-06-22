# Environment Setup & Installation Guide (Windows)

This guide walks you through setting up a complete development and execution environment for **docx2tex** on Windows using Chocolatey. It covers installing Rust, the necessary LaTeX compilers, and building the project.

---

## 1. Install Chocolatey
Chocolatey is a package manager for Windows that makes installing development tools incredibly easy.

1. Open **PowerShell** as Administrator (Right-click Start -> Windows PowerShell (Admin)).
2. Run the following command to install Chocolatey:
   ```powershell
   Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
   ```
3. Close and reopen the Administrator PowerShell window.

---

## 2. Install Rust (via Rustup)
With Chocolatey installed, installing the Rust toolchain manager (`rustup`) is a single command. `rustup` is the official way to manage Rust versions and will automatically install `cargo` and `rustc`.

1. In your Administrator PowerShell, run:
   ```powershell
   choco install rustup.install -y
   ```
2. Close your PowerShell window and open a **new, normal PowerShell window** (non-admin) so your `PATH` environment variables refresh.
3. Verify the installation by checking the versions:
   ```powershell
   rustup --version
   rustc --version
   cargo --version
   ```

---

## 3. Install a LaTeX Compiler

`docx2tex` needs a LaTeX compiler to generate PDFs (`--compile-pdf`). You can choose either **Tectonic** (recommended for ease of use) or **MiKTeX/TeX Live** (traditional).

### Option A: Tectonic (Recommended)
Tectonic is a modern, self-contained LaTeX engine that automatically downloads packages on the fly. Since you already installed Rust, you can install Tectonic using Cargo:

```powershell
cargo install tectonic
```
*(Note: This might take a few minutes to compile. Once finished, `tectonic.exe` will automatically be added to your PATH).*

### Option B: MiKTeX (Traditional Alternative)
If you prefer a traditional LaTeX distribution that installs `pdflatex`, you can install MiKTeX via Chocolatey:

1. Open PowerShell as Administrator and run:
   ```powershell
   choco install miktex -y
   ```
2. You may need to open the MiKTeX Console once after installation to check for updates and initialize the system.

---

## 4. Build the Project

With Rust and your LaTeX compiler installed, you are ready to build the `docx2tex` binary!

1. Clone the repository (if you haven't already):
   ```powershell
   git clone https://github.com/your-org/latex_generator
   cd latex_generator/latex
   ```
2. Build the optimized release binary:
   ```powershell
   cargo build --release
   ```

Your freshly built binary will be available at:
`target\release\docx2tex.exe`

---

## 5. Verify Your Setup

To test that everything is working flawlessly, run the conversion on a sample Word document and compile it straight to PDF:

```powershell
# Add the binary to your current session's PATH for convenience
$env:PATH += ";${PWD}\target\release"

# Run the tool!
docx2tex convert "C:\path\to\your\document.docx" --template default --output ./out --compile-pdf
```

If successful, you will see `document.tex` and `document.pdf` instantly appear in your `./out` directory!
