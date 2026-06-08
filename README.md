# Jconduit

A tool for bridging C/Rust native code to the Java Foreign Function & Memory (FFM) API. Its main
goal is to automate the generation of a high-performance interface between native code and the JVM.
I originally built it to bridge native physics engines to the JVM, but it generalizes to any native
code.

The central feature is a deferred command buffer: commands are serialized into a shared per-thread
buffer and executed in batch when you call `flush()` or when the buffer runs out of space. That
said, you can also use it to generate a plain bridge with no buffering and only direct calls, or
any mix of the two.

## ⚠️ Status: Early Alpha

This crate is in active development. The interface may change, documentation is incomplete, and
there's a fair amount of refactoring still to do. If you find a bug, please open an issue.

## Prerequisites

- Rust (stable)
- Java 22+
- [jextract](https://jdk.java.net/jextract/) — must be installed and on your `PATH`

## Installation

```shell
cargo install --path .
```

or add it as a local dependency in your workspace.

## How it works

The best way to get a feel for it is to look at the `examples/` folder alongside this explanation.

You give Jconduit two C headers:

- `functions_header` — the function declarations you want to expose
- `typedef_header` — the struct and type definitions shared between Java and native. **All types
  referenced in your function signatures must be defined directly in this file** — no `#include`
  chaining.

From those, Jconduit runs the following pipeline:

1. Runs **bindgen** to generate a Rust FFI interface to your native code.
2. Generates a **Rust middleman**: a command dispatcher that reads the JVM's command buffer and
   calls into native, plus direct-call wrappers for functions that return values or are marked as
   direct.
3. Generates a **`_java_bindings.h`** header describing the middleman's interface.
4. Runs **jextract** on that header to produce the low-level Java bindings.
5. Generates a **Proxy class** — this is what you actually use from Java. It writes commands into
   a per-thread buffer, handles flushing, resizing, and alignment, and exposes direct calls for
   queries. The buffer is shared across instances on the same thread, so a `flush()` call will
   dispatch everything in the buffer regardless of which instance wrote it.

## Usage

Jconduit is configured via a `jconduit.toml` file. Once it's set up:

```shell
cargo jconduit generate
```

or point it at a specific config:

```shell
cargo jconduit generate --config path/to/jconduit.toml
```

---

## Configuration reference

### `[generator]` — required

Core metadata and I/O paths.

```toml
[generator]
crate_name = "your_crate_name"              # Name of the generated Rust crate.
proxy_class_name = "YourProxyClass"         # Name of the generated Java proxy class.
package = "com.example"                     # Java package for generated code.
functions_header = "./path/to/functions.h"  # Header with the function declarations to expose.
typedef_header = "./path/to/types.h"        # Header with shared type definitions (structs, enums).
prefix = "engine_"                          # Prefix used to allowlist functions automatically.
version = "0.1.0"                           # Your bridge version.
output_dir = "./generated/jconduit"         # Output directory. Defaults to "./generated/jconduit".
```

---

### `[filters]` — optional

Controls what bindgen picks up. See [bindgen's docs](https://rust-lang.github.io/rust-bindgen/) for
details.

```toml
[filters]
allowlist_functions = []
allowlist_types = []
allowlist_vars = []
allowlist_items = []
allowlist_files = []

# These defaults already block compiler internals and common POSIX noise.
blocklist_functions = []
blocklist_types = ["^__.*", "^va_list.*"]
blocklist_vars = ["^__.*", "^COBJMACROS$"]
blocklist_items = ["^__.*"]
blocklist_files = []

opaque_types = ["^opaque_.*"]
no_copy_types = []
no_debug_types = []
no_default_types = []
no_hash_types = []
```

---

### `[options]` — optional

Fine-tunes code generation behavior.

```toml
[options]
strip_prefix = ""  # Strip this prefix from C function names in the generated Rust code.

# Bindgen options
derive_copy = true
derive_default = false
use_core = false
c_naming_conversion = false
layout_tests = true

# Raw lines injected at the top of the generated Rust file.
raw_lines = ["#![allow(dead_code, non_camel_case_types, non_snake_case, non_upper_case_globals)]"]

# Functions matching any of these prefixes or containing any of these keywords are routed to
# the direct (synchronous) execution path instead of the command buffer.
direct_prefixes = ["get_", "fetch_", "jconduit_direct_"]
direct_keywords = ["_get_"]

# If a void function's last parameter matches one of these suffixes or prefixes, Jconduit
# generates a convenience overload that allocates a scratch buffer, calls the function, and
# returns the result directly — so you don't have to manage the output pointer yourself.
# Example: `void body_get_rotation(uint32_t id, Quaternion *rotation_out)` becomes
# `MemorySegment bodyGetRotation(int id)`.
# Only one output parameter per function is supported, and it must be the last parameter.
output_param_prefix = []
output_param_suffix = ["_out", "_dest"]
```

---

### `[proxy_settings]` — optional

Command buffer sizing and lifecycle policy.

```toml
[proxy_settings]
min_buffer_size = 65536          # Minimum buffer size in bytes (64KB).
max_buffer_size = 2097152        # Maximum buffer size in bytes (2MB).

# The buffer shrinks if usage stays below `decaying_usage_threshold` for
# `decaying_frames_threshold` consecutive flush() calls.
decaying_frames_threshold = 300  # Flushes to wait before shrinking.
decaying_usage_threshold = 0.5   # Shrink if usage stays below 50%.
shrink_rate = 0.75               # Shrink the buffer to 75% of its current size.

# When a command doesn't fit, the buffer is flushed first, then grown by this factor.
growth_rate = 1.50               # Grow by 150% on overflow.

# If true, Jconduit manages the Arena lifecycle automatically (GC-managed).
# Disable this if you manage your own threads and want deterministic cleanup.
auto_arena = true
```