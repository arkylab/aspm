# aspm - AI Skill Package Manager

A Git-based package manager designed for AI-assisted development, similar to npm but supporting skills, agents, commands, hooks, and any AI resource types.

## Features

- 📦 **Two Project Modes**: Publish project (`aspub.yaml`) and consumer project (`aspkg.yaml`)
- 🔗 **Distributed Dependency Management**: Reference packages directly via Git URL, no central registry needed
- 🏷️ **Flexible Version Control**: Support for Git tag/branch/commit
- 📥 **Simplified Version Rules**: Auto-selects the maximum version satisfying all dependencies
- 🔧 **Universal Design**: Not limited to skills, supports any AI resource type
- 🔌 **Multi-Format Support**: Install both aspm packages and Claude Code plugin repositories

## Installation

### Quick Install

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/arkylab/aspm/main/scripts/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/arkylab/aspm/main/scripts/install.ps1 | iex
```

### Manual Download

Download the latest release for your platform:

| Platform | Architecture | Download |
|----------|-------------|----------|
| Linux | x64 | `aspm-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | ARM64 | `aspm-aarch64-unknown-linux-gnu.tar.gz` |
| macOS | Intel | `aspm-x86_64-apple-darwin.tar.gz` |
| macOS | Apple Silicon | `aspm-aarch64-apple-darwin.tar.gz` |
| Windows | x64 | `aspm-x86_64-pc-windows-msvc.zip` |
| Windows | ARM64 | `aspm-aarch64-pc-windows-msvc.zip` |

### Build from Source

```bash
git clone https://github.com/arkylab/aspm.git
cd aspm
cargo build --release
```

The compiled binary will be at `target/release/aspm` (or `aspm.exe` on Windows).

## Quick Start

### Creating a Consumer Project (If you are a skill consumer)

```bash
# Initialize a consumer project
aspm init --consumer

# This creates aspkg.yaml
```

#### Configure aspkg.yaml

```yaml
# Installation target directory
install_to: 
  - .claude

dependencies:
  my-skill-pack:
    git: "https://github.com/user/my-skill-pack.git"
    tag: "v1.0.0"
```

#### Install Dependencies

```bash
aspm install
```

### Creating a Publish Project (If you are a skill provider)

Publish projects allow you to share your AI resources with others.

```bash
# Initialize a publish project
aspm init my-skill-pack

# This creates aspub.yaml (publish configuration)
```

#### Configure aspub.yaml

```yaml
name: my-skill-pack
version: 1.0.0
description: "A pack of useful AI resources"
author: "Your Name"
license: MIT

# Install target for this package's own dependencies (optional)
install_to:
  - .claude

# Dependencies (optional)
dependencies:
  core-utils:
    git: "https://github.com/user/utils.git"
    tag: "v1.0.0"

# Resources to publish (paths relative to aspub.yaml location)
publish:
  skills:
    - skills/brainstorming/
    - skills/writing-plans.md
  commands:
    - commands/code-review.md
```

#### Create Your Skills

The directory structure is fully customizable via `aspub.yaml`:

```yaml
# aspub.yaml
name: my-skill-pack
version: 1.0.0

# Publish specific resources with optional regex patterns
# Paths are relative to aspub.yaml location
publish:
  skills:
    - skills/brainstorming/      # match directory (trailing /)
    - skills/writing-plans.md      # match file
    - "skills/^test-.*/"         # regex: match directories starting with test-
  commands:
    - commands/code-review.md       # match file
```

Corresponding directory structure:

```
my-skill-pack/
├── aspub.yaml
├── skills/
│   ├── brainstorming/
│   │   └── SKILL.md
│   ├── writing-plans.md
│   └── test-helpers/           # matched by "^skills/test-.*/"
└── commands/
    └── code-review.md             # file (no trailing /)
```

**Publish Path Rules:**

| Pattern | Behavior |
|---------|----------|
| `skills/brainstorming` | Match `skills/brainstorming` file only |
| `skills/brainstorming/` | Match `skills/brainstorming/` directory only (trailing `/`) |
| `skills/^test-.*/` | Regex - match directories under `skills/` starting with `test-` |
| `commands/^.*\.md$` | Regex - match all `.md` files |

Regex is auto-detected when path contains metacharacters: `^ $ . * + ? [ ] ( ) { } | \`

## Supported Repository Formats

aspm supports two repository formats:

### 1. aspm Format (Recommended)

Repositories with `aspub.yaml` at root. This is the recommended format because:

- ✅ Explicit control over what gets published
- ✅ Clear package metadata (name, version, description)
- ✅ Support for selective publishing (only specified resources)
- ✅ Transitive dependency support

### 2. Claude Code Plugin Format

Repositories without `aspub.yaml` but with resource directories at root:

```
superpowers/
├── skills/
│   └── brainstorming/
│       └── SKILL.md
├── agents/
├── commands/
├── hooks/
└── rules/
```

Supported directories: `skills`, `agents`, `commands`, `hooks`, `rules`

#### Installing Claude Code Plugins

```yaml
# aspkg.yaml
dependencies:
  superpowers:
    git: "https://github.com/obra/superpowers.git"
    tag: "v4.1.1"
```

## Install Modes

aspm supports two installation modes:

### Plain Mode (Default)

Copies resources to `<target>/<type>/<pkg>/`:

```
.agents/
├── skills/
│   └── my-pack/
│       └── my-skill/
└── commands/
```

### Claude Mode

Copies entire repo to `<target>/-plugins/<pkg>/` and updates `settings.local.json`:

```
.claude/
├── -plugins/
│   └── my-pack/
│       ├── skills/
│       └── .claude-plugin/marketplace.json
└── settings.local.json
```

### Mode Configuration

```yaml
# Multiple targets with auto mode: .claude path → Claude mode, others → Plain mode
install_to:
  - .claude
  - .agents
# Or
# Explicit mode configuration
install_to:
  - path: .claude
    mode: claude
  - path: .agents
    mode: plain
```

## Installation Directory Structure

All packages are installed with namespace isolation to prevent conflicts:

```
.claude/
├── skills/
│   ├── superpowers/        # Package name as subdirectory
│   │   ├── brainstorming/
│   │   │   └── SKILL.md
│   │   └── writing-plans/
│   │   │   └── SKILL.md
│   └── my-skill-pack/      # Another package
│       └── brainstorming/
│           └── SKILL.md
├── agents/
│   └── superpowers/
└── commands/
    └── superpowers/
```

## CLI Commands

```bash
# Initialization
aspm init <name>              # Create a publish project
aspm init --consumer          # Create a consumer project

# Dependency Management
aspm install                  # Install all dependencies
aspm install --to <dir>       # Install to specific directory

# Cache Management
aspm cache clean              # Clear all cached repositories
aspm cache dir                # Show cache directory
aspm cache list               # List cached repositories
```

## Configuration Files

### Publish Project (aspub.yaml)

```yaml
name: my-skill-pack
version: 1.0.0
description: "A pack of useful AI resources"
author: "Your Name"
license: MIT

# Install target for this package's own dependencies
# Required if you have dependencies defined below
install_to:
  - .claude

# Resources to publish (paths relative to aspub.yaml location)
# Supports regex patterns (auto-detected by metacharacters)
publish:
  skills:
    - skills/brainstorming/      # match directory (trailing /)
    - skills/writing-plans.md      # match file
    - "skills/^test-.*/"         # regex: match directories starting with test-
  commands:
    - commands/code-review.md       # match file

# Dependencies (optional)
dependencies:
  core-utils:
    git: "https://github.com/user/utils.git"
    tag: "v1.0.0"
```

### Consumer Project (aspkg.yaml)

```yaml
# Multiple targets (dependencies copied to all)
install_to:
  - .claude
  - .agents
# Or
# Explicit mode configuration
# install_to:
#   - path: .claude
#     mode: claude
#   - path: .agents
#     mode: plain

dependencies:
  my-skill-pack:
    git: "https://github.com/user/pack.git"
    tag: "v1.0.0"
```

## Version Rules

aspm uses simplified version rules:

- Auto-selects the **maximum version** satisfying all dependencies
- Tags/branches matching version format (e.g., `v1.0.0`) participate in version comparison

```yaml
dependencies:
  skill-a:
    git: "https://..."
    tag: "v1.2.0"      # Exact tag
  
  skill-b:
    git: "https://..."
    branch: "develop"  # Specific branch
  
  skill-c:
    git: "https://..."
    commit: "a1b2c3d4" # Exact commit
```

## Why Use aspub Format?

| Feature | aspub Format | Claude Plugin Format |
|---------|-------------|---------------------|
| Transitive dependencies | ✅ Automatic | ❌ Not supported |
| Dependency resolution | ✅ Automatic conflict resolution | ❌ Manual |

## License

MIT
