# comet
A virtual pen that can be grabbed and used to draw sketches quickly anywhere within Stardust XR.
> [!IMPORTANT]  
> Requires the [Stardust XR Server](https://github.com/StardustXR/server) to be running.

If you installed the Stardust XR server via:  
```note
sudo dnf group install stardust-xr
```
Or if you installed via the [installation script](https://github.com/cyberneticmelon/usefulscripts/blob/main/stardustxr_setup.sh), Atmosphere comes pre-installed


## How to Use
Run the command `comet` or `comet_dev`

### Controllers
Put the cursor near the pen and hold grip to grab it. Then, hold trigger to draw.

### Hands
Curl your middle, ring, and pinky fingers when your hand is next to the pen to grab it. It should look as if holding a pen. Then pinch index and thumb together to draw, and unpinch to stop drawing.

### Pointers
Not supported yet

## Manual Installation
Clone the repository and after the server is running:
```sh
cargo run
```


## Todo
- Add erase functionality (ideally making a fist and rubbing out pen marks like on a whiteboard for hands)
- Improve stroke stability of the pen
- Add signifiers for when in range to grab the pen and how to grab the pen
- Support pointers (unsure of how to do it given they don't really have a reliable 3D pose)
