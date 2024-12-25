![**Hoolamike**](./media/hoolamike-logo.png)

# 🏄 Hoolamike: Wabbajack Modlist Installer for Linux 

Hoolamike is here to ease the process of installing **Wabbajack modlists** on Linux without the hassle of virtual machines or Wine. This project ports the functionality on a **per-modlist basis**, enabling seamless integration with your favorite games. 🌟

## 🚀 Current Support Focus

- [**Tuxborn**](https://github.com/Omni-guides/Tuxborn): A Wabbajack Modlist for Skyrim SE designed with the Steam Deck in mind 🐉  
- [**Wasteland Reborn**](https://github.com/Camora0/Wasteland-Reborn): modlist that aims to create a fully featured, lore friendly rebuild of Fallout 4 from the ground up.  ☢️  

## 🔮 Wishlist & Community Support

Want a specific modlist to be supported? 🤔  
Join our **[Discord Community](https://discord.gg/xYHjpKX3YP)** and let us know! Your feedback and suggestions drive this project forward. 💬

## 💡 Goals

Wabbajack modlist installation logic is being slowly ported from C# to Rust, while keeping linux support constantly in mind.  🛠️
## ❌ Non-goals 
Replacing Wabbajack, modlist creation - this project focuses only on installation of modlists keeping them compatible with Wabbajack.

## 🙏 Special Thanks

A huge shoutout to these amazing libraries that make Hoolamike possible:

- [bsa-rs](https://github.com/Ryan-rsm-McKenzie/bsa-rs) 🗂️: Efficient handling of BSA archives.
- [image_dds](https://github.com/ScanMountGoat/image_dds) 🖼️: Processing DDS image formats with style.

Make sure to ⭐ star their repositories — their work is the backbone of this project!

## ⭐ Features

- 🐧 **Linux Native**: Say goodbye to VMs or Wine setup, enjoy quick and streamlined installation process.  
- ⚡ **Optimized for Performance**: Parallelization of installation steps is one of the main focuses. As much as possible is performed in a multithreaded fashion to lower the wait time.
- 📚 **Per-Modlist Focus**: Tailored support for specific modlists.  

## 🏄 How to Get Started with Hoolamike
1. Install the Rust toolchain: Visit https://rustup.rs/ to install Rust.
2. Clone the Hoolamike repository: Run git clone https://github.com/Niedzwiedzw/hoolamike to download the project files.
3. Switch to the nightly Rust compiler: Run rustup default nightly to set the nightly version as default. This step is required because Hoolamike uses features available only in the nightly version of Rust.
4. Install Hoolamike using Cargo: Navigate to the repository and execute `cargo install --path crates/hoolamike`.
5. Verify the installation: Once installed, the binary will typically be located in ~/.cargo/bin/. Ensure the binary is in your system's $PATH, or reference it directly by running ~/.cargo/bin/hoolamike. You should see a help message indicating successful installation.
6. Configure Hoolamike:
    Run hoolamike print-default-config > hoolamike.yaml to generate a default configuration file.
    Edit hoolamike.yaml in a text editor. Add your Nexus API key, which you can obtain from https://next.nexusmods.com/settings/api-keys.
    Specify game directories, such as:
```
  games:
    Fallout4:
      root_directory: "/path/to/Fallout 4/"
````
7. Obtain the required modlist file: Download the <modlist-name>.wabbajack file for your desired modlist. You might need to check the Wabbajack community for the appropriate link. Place this file in the same directory as hoolamike.yaml.
8. Update the configuration: In hoolamike.yaml, set the path to the downloaded .wabbajack file under installation.wabbajack_file_path.
9. Install the modlist: Run `hoolamike install`. 

If you face any issues, consult the Hoolamike GitHub repository for further guidance or file a support ticket.


## 💬 Join the Community

Whether you're here to wishlist modlists, contribute, or just chat with fellow enthusiasts, our **[Discord Community](https://discord.gg/xYHjpKX3YP)** is open for you! 🎉

## 🌟 Contributing

We welcome contributions of all kinds! Whether it’s fixing bugs, improving documentation, or adding support for new modlists, your help is appreciated. Check out our `CONTRIBUTING.md` for guidelines.

---

Hoolamike is built **by Linux gamers, for Linux gamers**. Let's make modding on Linux better together! 🐧✨
