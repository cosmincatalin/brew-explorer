[![Built With Ratatui](https://ratatui.rs/built-with-ratatui/badge.svg)](https://ratatui.rs/)

# 🍺 Brew Explorer

A beautiful terminal UI for exploring and managing your Homebrew packages with ease.

## What is Brew Explorer?

You know how after some time you run `brew list` and see a bunch of packages you have no idea about? Me too! It's frustrating to figure out what those packages do without copy/pasting their names into [brew.sh](https://brew.sh) and only then discovering what they're good for.  

That's why I created **Brew Explorer** - a little app that makes it easy to understand what formulaes and casks you have installed on your system.   
Plus, you can quickly **uninstall old useless things** and **update whatever was left behind** - all from within the same interface!

But why stop at Homebrew? If you're a Mac user, you likely have apps from the **Mac App Store** too. That's where [**mas**](https://github.com/mas-cli/mas) comes in — a brilliant command-line interface for the Mac App Store that brings it under the same terminal-native workflow. Brew Explorer integrates with `mas` to give you a unified view of your Mac software, whether it came from Homebrew or the App Store — though it works perfectly fine without it too.

## Screenshots

### Loading Screen
![Loading Screen](.resources/loading_screen.png)

### Main Screen

![Main Screen](.resources/main_screen.png)

### Search and Uninstall

![Uninstall](.resources/uninstall.png)

## Features

✨ **Appealing TUI** - Clean, intuitive terminal interface with ASCII art  
🔍 **Search functionality** - Quickly find packages by name  
📦 **Package details** - View descriptions, homepages, versions, and more  
🔄 **Update packages** - Update outdated packages with a single keystroke  
🗑️ **Uninstall packages** - Remove packages you no longer need  
🎨 **Color-coded status** - Green for up-to-date, red for outdated packages  
⚡ **Fast navigation** - Multi-column layout for efficient browsing  
🚀 **Auto-sync** - Runs `brew update` on startup for fresh package information  
🍎 **Mac App Store support** - Browse and manage App Store apps via [mas](https://github.com/mas-cli/mas)

## Installation

### Prerequisites

- macOS (Homebrew is macOS-specific)
- [Homebrew](https://brew.sh) installed
- [mas](https://github.com/mas-cli/mas) (`brew install mas`) — optional, enables Mac App Store integration

### Steps

```shell
brew tap cosmincatalin/tap
brew install brew-explorer
```

### Navigation

- **↑/↓** or **k/j** - Move up/down in the package list (Vim-style)
- **←/→** or **h/l** - Move left/right between columns (Vim-style)
- **PgUp/PgDn** - Jump by pages
- **Home/End** - Jump to first/last package

### Actions

- **/** - Start searching for packages
- **u** - Update the selected package
- **x** - Uninstall the selected package
- **q** - Quit the application

### Search Mode

- Type any character to search for packages by name
- **Enter** or **Esc** - Exit search and return to browsing
- Navigation keys (↑/↓/←/→ or k/j/h/l) still work while searching
