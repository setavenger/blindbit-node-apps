## Minimum VM Requirements
- **RAM:** 4GB. (8GB+ recommended)
- **Storage:** 32GB. (Storage needs depend on your use case. For instance, running a Bitcoin node typically requires 1TB or more, while file/media storage needs can be estimated based on your cloud services usage.)

## How to install

#### Step 1: Download the installer ISO
- Download the latest umbrelOS USB installer [umbrelos-amd64-usb-installer.iso](https://download.umbrel.com/release/latest/umbrelos-amd64-usb-installer.iso) on your computer.

> [!NOTE]
> The following steps to create and boot a VM are general guidelines. Please refer to your VM software's documentation for specific instructions.

#### Step 2: Create a new VM and attach the installer ISO
- Create a new VM using your preferred virtualization software
- Allocate at least 4GB of RAM
- Attach a fresh virtual hard drive (minimum 32GB)
- Attach the decompressed .iso file from Step 1 to the VM's virtual CD-ROM drive or USB drive

#### Step 3: Boot the VM
- Set the the VM to boot from the attached installer ISO (e.g., if you attached the ISO to the CD-ROM drive, set the boot order to boot from CD-ROM first)
- Ensure the VM is set to boot in EFI mode and not legacy BIOS mode. The installer requires EFI mode to boot correctly.
- Boot the VM

#### Step 4: Install umbrelOS
- After your VM boots up, it will present you with the following screen:
![umbrelos-usb-installer-3](https://github.com/getumbrel/umbrel/assets/85373263/d6d54c7f-e1a0-4433-af68-da7ebcf24cd8)
- Identify the correct virtual hard drive where you intend to install umbrelOS, and note its corresponding number. For instance, in the above screenshot, if we want to install it on the 'ATA' drive, which has a capacity of '100G', we'd type '1' (without the quotes) and press Enter.

#### Step 5: Finalize installation
- After the installation is complete, shut down the VM
- Remove the installer ISO from the VM's virtual CD-ROM drive or USB drive

#### Step 6: Boot up!
- Boot the VM (**remember to remove the installer ISO first!**)
- The first boot will take ~5 minutes. After booting up, you can access umbrelOS by navigating to [http://umbrel.local](http://umbrel.local) on any device connected to the same network.

> [!NOTE]
> **Important**: You might notice the `umbrel login` prompt in the VM's terminal output. This prompt is for terminal access only. To use umbrelOS, there is no need to log in via the terminal. Instead, simply open [http://umbrel.local](http://umbrel.local) on any device that is connected to the same network to access the user interface.

> [!TIP]
> If you cannot access [http://umbrel.local](http://umbrel.local), and you are using a Windows computer, try accessing [http://umbrel](http://umbrel). Alternatively, use your router's admin dashboard or a network scanning tool like 'Fing' to find your device's IP address on your local network. If needed, you can connect a display and a keyboard to your device, log in, and run `hostname -I` to find its IP address. Then, enter this IP address directly into a web browser to access umbrelOS.

## Support

To get support or share your experience with others, please join our [community forum](https://community.umbrel.com) and [Discord server](http://discord.gg/efNtFzqtdx).