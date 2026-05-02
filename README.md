# aspm - AI Skill Package Manager

A Git-based package manager designed for AI-assisted development, similar to npm but supporting skills, agents, commands, hooks, and any AI resource types.

## Features

- 📦 **Two Project Modes**: Publish project (`aspub.yaml`) and consumer project (`aspkg.yaml`)
- 🔗 **Distributed Dependency Management**: Reference packages directly via Git URL, no central registry needed
- 🏷️ **Flexible Version Control**: Support for Git tag/branch/commit
- 📥 **Simplified Version Rules**: Auto-selects the maximum version satisfying all dependencies
- 🔧 **Universal Design**: Not limited to skills, supports any AI resource type
- 🔌 **Multi-Format Support**: Install both aspm packages and Claude Code plugin repositories
- 🎯 **Three Install Modes**: Plain, Claude, and Compatible modes for different AI tool directory structures

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
  - .claude    # Install to Claude Code plugin directory

dependencies:
  superpowers:
    git: "https://github.com/obra/superpowers.git"
    branch: "main"
```

#### Install Dependencies

```bash
aspm install
# After running `aspm install`, all dependencies are ready. If you are using Claude Code, you can now restart Claude code to load the new skills. Sometimes, you may need to restart it twice.
```

**✅ That's all you need to do as a skill consumer**

### Creating a Publish Project (If you are a skill provider)

Publish projects allow you to share your AI resources with others.

**Supported Repository Formats:**

| Format | Description | Recommended |
|--------|-------------|-------------|
| **aspm Format** | Repository with `aspub.yaml` at root | ✅ Yes |
| **Claude Plugin Format** | Repository with `skills/`, `agents/`, etc. directories at root | ⚠️ No |
| **Single Skill Format** | Repository with only `SKILL.md` at root | ⚠️ No |

aspm recommends the **aspm Format** because it provides:
- ✅ Explicit control over what gets published
- ✅ Support for transitive dependencies
- ✅ Automatic dependency resolution

```bash
# Initialize a publish project
aspm init my-skill-pack
# This creates aspub.yaml (publish configuration).
# aspub.yaml and aspkg.yaml can coexist in the same project - one for publishing your own resources, one for consuming dependencies.
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

aspm supports three repository formats:

### 1. aspm Format (Recommended)

Repositories with `aspub.yaml` at root. This is the recommended format because:

- ✅ Explicit control over what gets published
- ✅ Support for selective publishing (only specified resources)
- ✅ Transitive dependency support

### 2. Claude Code Plugin Format

Repositories without `aspub.yaml` but with resource directories at root:

```
superpowers/
├── .claude-plugin/
│   └── marketplace.json
├── skills/
│   └── brainstorming/
│       └── SKILL.md
├── agents/
├── commands/
├── hooks/
└── rules/
```

#### Installing Claude Code Plugins

```yaml
# aspkg.yaml
dependencies:
  superpowers:
    git: "https://github.com/obra/superpowers.git"
    branch: "main"
```

### 3. Single Skill Format

Repositories with only a `SKILL.md` file at root (no standard directories). aspm auto-wraps it in a `skills/` directory structure during installation.

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

**Note:** Supported directories: `skills`, `agents`, `commands`, `hooks`, `rules`

### Claude Mode

Copies entire repo to `<target>/-plugins/<pkg>/` and updates `settings.local.json`:

```
.claude/
├── -plugins/
│   └── my-pack/
│       ├── .claude-plugin/
│       │   └── marketplace.json
│       └── skills/
└── settings.local.json
```

**Note:** If the source repository lacks `.claude-plugin/marketplace.json`, aspm auto-generates it with the package name as marketplace name (suffixed with `-dev`).

### Compatible Mode

Copies resources to `<target>/<type>/<skill>/` without the package name layer, for compatibility with AI tools like Qwen that expect a flat resource directory structure:

```
.qwen/
├── skills/
│   └── brainstorming/        # No package name layer
│       └── SKILL.md
└── commands/
```

**Note:** Auto-detected when `install_to` path ends with `.qwen`. Existing resources with the same name are skipped with a warning to prevent overwriting.

### Mode Configuration

```yaml
# Multiple targets with auto mode: .claude → Claude mode, .qwen → Compatible mode, others → Plain mode
install_to:
  - .claude
  - .qwen
  - .agents
# Or
# Explicit mode configuration
# install_to:
#   - path: .claude
#     mode: claude
#   - path: .qwen
#     mode: compatible
#   - path: .agents
#     mode: plain

dependencies:
  superpowers:
    git: "https://github.com/obra/superpowers.git"
    branch: "main"
```

## Installation Directory Structure

All packages are installed with namespace isolation to prevent conflicts. Example with `install_to: [.claude, .agents]`:

```
.claude/                          # Claude mode (auto-detected)
├── -plugins/
│   └── superpowers/              # Package name
│       ├── commands/
│       ├── skills/
│       │   ├── brainstorming/
│       │   │   └── SKILL.md
│       │   └── writing-plans/
│       │       └── SKILL.md
│       └── .claude-plugin/
│           └── marketplace.json
└── settings.local.json           # Updated with plugin paths

.agents/                          # Plain mode (auto-detected)
├── skills/
│   └── superpowers/              # Package name as subdirectory
│       ├── brainstorming/
│       │   └── SKILL.md
│       └── writing-plans/
│           └── SKILL.md
└── commands/
    └── superpowers/
```

## CLI Commands

```bash
# Initialization
aspm init <name>              # Create a publish project
aspm init --consumer          # Create a consumer project

# Dependency Management
aspm install                                                # Install all dependencies
aspm install --to <dir>       # Install to specific directory
aspm install --to .qwen::compatible  # Install to .qwen with compatible mode
aspm install --extra <file>   # Merge extra config
aspm install --aspkg <file>   # Use custom aspkg.yaml path
aspm install --extra local.yaml --to .cursor --aspkg ./config/aspkg.yaml  # Combined options
aspm add <name> --git <url>                        # Add dependency (auto-detect default branch)
aspm add <name> --git <url> [--branch | --tag | --commit] <ref>  # Add with specific branch or tag or commit
aspm add <name> --git <url> --aspkg <file>          # Add to specific aspkg.yaml
aspm add <name> --git <url> --aspub                 # Add to aspub.yaml
aspm add <name> --git <url> --overwrite             # Overwrite existing dependency
aspm remove <name>            # Remove dependency
aspm remove <name> --aspkg <file>  # Remove from specific aspkg.yaml
aspm remove <name> --aspub         # Remove from aspub.yaml

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
# Global install targets (used if dependency has no own install_to)
install_to:
  - .claude

dependencies:
  my-skill-pack:
    git: "https://github.com/user/pack.git"
    tag: "v1.0.0"
    # Optional: override global install_to for this dependency
    install_to:
      - .cursor
```

#### Extra Config File

Use `--extra` to merge additional dependencies (extra file overrides aspkg.yaml):

```yaml
# extra.yaml
install_to:
  - .cursor

dependencies:
  my-skill-pack:
    git: "https://github.com/user/pack.git"
    branch: develop
    install_to:
      - .cursor
```

```bash
aspm install --extra extra.yaml
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

## License

MIT
