# komokana

Automatic application-aware keyboard layer switching for Windows

# About

`komokana` is a daemon that listens to events emitted by [`komorebi`](https://github.com/LGUG2Z/komorebi) and communicates
with [`kanata`](https://github.com/jtroo/kanata) to switch keyboard layers based on a set of user defined rules.

`komokana` allows you associate different `kanata` keyboard layers with specific applications, and automatically switch
to that keyboard layer when the windows of those applications are focused in the foreground.

You may join the `komorebi` [Discord server](https://discord.gg/mGkn66PHkx) for any `komokana`-related discussion, help,
troubleshooting etc. If you have any specific feature requests or bugs to report, please create an issue in this repository.

Articles, blog posts, demos and videos about `komokana` can be added to this section of the readme by PR.

# Description

`komokana` communicates with `komorebi`
using [Named Pipes](https://docs.microsoft.com/en-us/windows/win32/ipc/named-pipes),
and with `kanata` via a [TCP server](https://en.wikipedia.org/wiki/Transmission_Control_Protocol) that can be optionally
started by passing the `--port` flag when launching the `kanata` process.

If either the `komorebi` or `kanata` processes are stopped or killed, `komokana` will attempt to reconnect to them
indefinitely. However, `komokana` will not launch successfully if either one of those processes is not running.

# Getting Started

## Prerequisites

- The latest version of `komorebi`
  - `scoop install komorebi` (from the `extras` bucket)
- The latest version of `kanata`
  - `cargo install kanata`

## GitHub Releases

Prebuilt binaries of tagged releases are available on the [releases page](https://github.com/LGUG2Z/komokana/releases)
in a `zip` archive.

Once downloaded, you will need to move the `komokana.exe` binary to a directory in your `Path` (
you can see these directories by running `$Env:Path.split(";")` at a PowerShell prompt).

Alternatively, you may add a new directory to your `Path`
using [`setx`](https://docs.microsoft.com/en-us/windows-server/administration/windows-commands/setx) or the Environment
Variables pop up in System Properties Advanced (which can be launched with `SystemPropertiesAdvanced.exe` at a
PowerShell prompt), and then move the binaries to that directory.

## Scoop

If you use the [Scoop](https://scoop.sh/) command line installer, you can run
the following commands to install the binaries from the latest GitHub Release:

```powershell
scoop bucket add extras
scoop install komokana
```

If you install _komokana_ using Scoop, the binary will automatically be added
to your `Path`.

## Building from Source

If you prefer to compile _komokana_ from source, you will need
a [working Rust development environment on Windows 10](https://rustup.rs/). The `x86_64-pc-windows-msvc` toolchain is
required, so make sure you have also installed
the [Build Tools for Visual Studio 2019](https://stackoverflow.com/a/55603112).

You can then clone this repo and compile the source code to install the binary for `komokana`:

```powershell
cargo install --path . --locked
```

## Configuring

`komokana` is configured using a YAML file that can be specified using the `-c` flag.

Consider the following `kanata.kbd` file which defines our keyboard layers:

```clojure
(defalias
  ;; these are some convenient aliases to send the letter on tap, or toggle the
  ;; "firefox" layout on hold
  ft   (tap-hold 50 200 f (layer-toggle firefox))
  jt   (tap-hold 50 200 j (layer-toggle firefox))

  ;; these are some convenient aliases for us to switch layers
  qwr  (layer-switch qwerty)
  ff   (layer-switch firefox)
)

;; imagine this is our default layer, passed as "-d qwerty" when launching komokana
;; the only two keys overriden here are f and j, which when held, will toggle
;; our "firefox" layer
(deflayer qwerty
  _    _    _    _    _    _    _    _    _    _    _    _    _          _    _    _
  _    _    _    _    _    _    _    _    _    _    _    _    _    _     _    _    _
  _    _    _    _    _    _    _    _    _    _    _    _    _    _     _    _    _
  _    _    _    _    @ft  _    _    @jt  _    _    _    _    _
  _    _    _    _    _    _    _    _    _    _    _    _    _               _
  _    _    _              _                   _    _         _          _    _    _
)

;; this is our firefox layer which lets us navigate webpages using hjkl
(deflayer firefox
  _    _    _    _    _    _    _    _    _    _    _    _    _          _    _    _
  _    _    _    _    _    _    _    _    _    _    _    _    _    _     _    _    _
  _    _    _    _    _    _    _    _    _    _    _    _    _    _     _    _    _
  _    _    _    _    _    _    left down up   rght _    _    _
  _    _    _    _    _    _    _    _     _    _   _    _    _               _
  _    _    _              _                    _   _         _          _    _    _
)

;; this is our editor layer for use in windows where the vim editor or vim extensions
;; in a text editor are running. the only thing we do here is ensure that the tap-hold
;; not present on j, so that when we hold down j we can zoom all the way down the file
;; that we are editing
(deflayer editor
  _    _    _    _    _    _    _    _    _    _    _    _    _          _    _    _
  _    _    _    _    _    _    _    _    _    _    _    _    _    _     _    _    _
  _    _    _    _    _    _    _    _    _    _    _    _    _    _     _    _    _
  _    _    _    _    @ft  _    _    _    _    _    _    _    _
  _    _    _    _    _    _    _    _    _    _    _    _    _               _
  _    _    _              _                   _    _         _          _    _    _
)
```

Based on the `kanata` layers defined above, we can have a `komokana.yaml` configuration file that looks like this:

```yaml
- exe: "firefox.exe" # when a window with this exe is active
  target_layer: "firefox" # switch to this layer, a vim-like layer just for browsing!
  title_overrides: # unless...
    - title: "Slack |" # the window title matches this
      # valid matching strategies are: starts_with, ends_with, contains and equals
      strategy: "starts_with" # matching with this matching strategy
      target_layer: "qwerty" # if it does, then switch to this layer for chatting
    - title: "Mozilla Firefox" # new firefox tab, we'll probably want to switch to qwerty mode to type a url!
      strategy: "equals"
      target_layer: "qwerty"
  virtual_key_overrides: # unless...
    # list of key codes and their decimal values here: https://cherrytree.at/misc/vk.htm
    - virtual_key_code: 18 # this key is held down (alt in this case) when the window becomes active
      targer_layer: "qwerty" # if it is, then switch to this layer, so that we can continue switching window focus with alt+hjkl
  virtual_key_ignores: # alternatively
    - 18 # if this key is held down (alt in this case), then don't make any layer switches

# your normal layer might have a tap-hold on j since it's a such convenient and ergonomic key
# but it sucks to be in vim, holding down j to move down and have nothing happen because of the hold...
# no worries! let's just switch to a layer which removes the tap-hold on the j when we are in windows
# where we use vim or vim editing extensions!
- exe: "WindowsTerminal.exe"
  target_layer: "editor"
- exe: "idea64.exe"
  target_layer: "editor"
```

## Running

Once you have either the prebuilt binaries in your `Path`, or have compiled the binaries from source (these will already
be in your `Path` if you installed Rust with [rustup](https://rustup.rs), which you absolutely should), you can
run `komokana -p [KANATA_PORT] -d [DEFAULT_LAYER] -c [PATH_TO_YOUR_CONFIG]` at a Powershell prompt, and you should start to see log output.

Remember, both `komorebi` and `kanata` must be running before you try to start `komokana`, and `kanata` must be running
with the `--port` flag to enable the TCP server on the given port.

This means that `komokana` is now running and listening for notifications sent to it by `komorebi`.

### `yasb` Widget

When running `komokana` with the `-t` flag, a plaintext file will be updated whenever the layer changes at the following
location: `~/AppData/Local/Temp/kanata_layer`

You may optionally use this file to construct a simple [`yasb`](https://github.com/denBot/yasb) widget which polls and
displays the contents of that file to provide a visual indicator of the currently :

```yaml
# in ~/.yasb/config.yaml
widgets:
  kanata:
    type: "yasb.custom.CustomWidget"
    options:
      label: "{data}"
      label_alt: "{data}"
      class_name: "kanata-widget"
      exec_options:
        run_cmd: "cat '%LOCALAPPDATA%\\Temp\\kanata_layer'"
        run_interval: 300
        return_format: "string"
```

# Contribution Guidelines

If you would like to contribute to `komokana` please take the time to carefully read the guidelines below.

## Commit hygiene

- Flatten all `use` statements
- Run `cargo +stable clippy` and ensure that all lints and suggestions have been addressed before committing
- Run `cargo +nightly fmt --all` to ensure consistent formatting before committing
- Use `git cz` with
  the [Commitizen CLI](https://github.com/commitizen/cz-cli#conventional-commit-messages-as-a-global-utility) to prepare
  commit messages
- Provide **at least** one short sentence or paragraph in your commit message body to describe your thought process for the
  changes being committed

## License

`komokana` is licensed under the [Komorebi 1.0.0 license](./LICENSE.md), which
is a fork of the [PolyForm Strict 1.0.0
license](https://polyformproject.org/licenses/strict/1.0.0). On a high level
this means that you are free to do whatever you want with `komokana` for
personal use other than redistribution, or distribution of new works (i.e.
hard-forks) based on the software.

Anyone is free to make their own fork of `komokana` with changes intended
either for personal use or for integration back upstream via pull requests.

_The [Komorebi 1.0.0 License](./LICENSE.md) does not permit any kind of
commercial use._

### Contribution licensing

Contributions are accepted with the following understanding:

- Contributed content is licensed under the terms of the 0-BSD license
- Contributors accept the terms of the project license at the time of contribution

By making a contribution, you accept both the current project license terms, and that all contributions that you have
made are provided under the terms of the 0-BSD license.

#### Zero-Clause BSD

```
Permission to use, copy, modify, and/or distribute this software for
any purpose with or without fee is hereby granted.

THE SOFTWARE IS PROVIDED “AS IS” AND THE AUTHOR DISCLAIMS ALL
WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES
OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE
FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY
DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN
AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT
OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```
