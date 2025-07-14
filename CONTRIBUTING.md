# Contributing
Thank you for your interest in contributing, I really appreciate it.

## Contributing Code

Feel free to open a Pull Request if you have a feature you'd like to add to this bot. Whether it's a small fix or a major improvement, contributions of all sizes are welcome.

## Questions & Discussions

If you have any questions about the codebase or want to discuss an idea before submitting a PR, please use the **Discussions** tab on GitHub.

## Building the bot

To build the bot, you need to have [dioxus cli](https://dioxuslabs.com/cli/) installed. You also need opencv. This has not been tested on WSL2.
```powershell
# On Windows, you can use vcpkg to install opencv
vcpkg install opencv4[contrib,nonfree]:x64-windows-static
```

Once you have it, you can run the following command in the root directory of the project:
```powershell
dx build --release --package ui # CPU backend
dx build --release --package ui -- --features backend/gpu # GPU backend
```
