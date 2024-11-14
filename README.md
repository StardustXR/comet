# comet
A virtual pen that can be grabbed and used to draw sketches quickly anywhere within Stardust XR.

## Support/Questions
Discord: https://discord.gg/PV5CW6Y4SV
Matrix Space: #stardust-xr:matrix.org

## Run
1. Run Monado or WiVRn
2. Run the [Stardust XR server](https://github.com/StardustXR/server/)
3. `cargo run --locked`

## Usage
### Controllers
Put the cursor near the pen and hold grip to grab it. Then, hold trigger to draw.

### Hands
Curl your middle, ring, and pinky fingers when your hand is next to the pen to grab it. It should look as if holding a pen. Then pinch index and thumb together to draw, and unpinch to stop drawing.

### Pointers
Not supported yet

## Todo
- Add erase functionality (ideally making a fist and rubbing out pen marks like on a whiteboard for hands)
- Improve stroke stability of the pen
- Add signifiers for when in range to grab the pen and how to grab the pen
- Support pointers (unsure of how to do it given they don't really have a reliable 3D pose)
