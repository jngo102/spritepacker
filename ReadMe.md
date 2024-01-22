<h1 align='center'><b>Sprite Packer</b></h1>
<p align='center'>
    <a href="https://github.com/jngo102/spritepacker/actions/workflows/main.yml">
        <img src="https://img.shields.io/github/actions/workflow/status/jngo102/spritepacker/main.yml?branch=main"
            alt="spritepacker build status">
    </a>
    <a href="https://github.com/jngo102/spritepacker">
        <img src="https://img.shields.io/github/downloads/jngo102/spritepacker/total"
            alt="spritepacker build status">
    </a>
    <a href="https://github.com/jngo102/spritepacker/commits">
        <img src="https://img.shields.io/github/commit-activity/m/jngo102/spritepacker"
            alt="spritepacker commit frequency">
    </a>
    <a href="https://github.com/jngo102/spritepacker/blob/main/License.md">
        <img src="https://img.shields.io/github/license/jngo102/spritepacker"
            alt="spritepacker software license">
    </a>
</p>
<p align='center'>
    <a href="https://discord.gg/VDsg3HmWuB">
        <img src="https://img.shields.io/discord/879125729936298015?logo=discord"
            alt="Visit the Hollow Knight Modding Discord server">
    </a>
    <a href="https://twitter.com/intent/follow?screen_name=JngoCreates">
        <img src="https://img.shields.io/twitter/follow/JngoCreates?style=social&logo=twitter"
            alt="Follow JngoCreates on Twitter">
    </a>
</p>

![spritepacker screenshot](/images/window.png)

This is a re-implementation of [HollowKnight.SpritePacker](https://github.com/magegihk/HollowKnight.SpritePacker), originally created by [magegihk](https://github.com/magegihk). It is built on the [egui](https://www.egui.rs/) framework.

## **Installation**

1. Navigate to the [Releases](https://github.com/jngo102/spritepacker/releases) page.
2. Download the latest release for your platform.

## **Usage**

1.  When you first open the app, you will be asked to choose a folder location. This will be where your sprites are stored (_Note_: _NOT_ an animation folder containing the PNG files!). You can change this location at any time by modifying the text box in the top panel, or by clicking on the "Browse" button.
2.  Before packing, you must check that each sprite and its duplicates are identical by clicking on the "Check" button at the bottom. Any sprites that are not identical will appear in the "Changed Sprites" list on the right. You can then click the sprite that you want to replace all duplicates with and then click on the "Replace Duplicates" button to replace them.
3.  After packing, a file dialog will open to ask where to save the generated atlas.

## **Issues**

If you encounter any issues, please report them on the [Issues](https://github.com/jngo102/spritepacker/issues) page.
