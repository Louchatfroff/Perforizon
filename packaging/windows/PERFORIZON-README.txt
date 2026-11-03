Perforizon: a performance-first fork of Horizon (Minecraft Java 26.3 in Rust)
==============================================================================

Perforizon is an unofficial fan project, not affiliated with Mojang Studios or
Microsoft. It contains none of Minecraft's code, textures or sounds: you
prepare them from your own copy of Minecraft Java Edition (see PLAY.txt).
World generation is identical to Horizon's: the same seed gives the same world.

Start
  1. Prepare the game files once (PLAY.txt has the details):
       horizon-prepare.exe --prepare --server-jar PATH\TO\server.jar
       horizon-prepare.exe sounds
  2. Run horizon-client.exe. Options: horizon-client.exe --help.

What Perforizon adds
  Autotune
    In a world, run:  horizon-ctl.exe autotune 240   (240 = the frame rate to aim for)
    It tries the performance settings for about a minute, keeps the best,
    lowers the resolution scale until the target is met, saves the options and
    writes autotune.json in the game folder.

  PvP options (file horizon-pvp.txt in the game folder, also in the options screen)
    Hit prediction (+ sound), crystal and anchor optimizers, elytra optimizer,
    toggle sneak status, hitboxes (colors, behind walls, eye line),
    projectile trails, damage flash, target-colored crosshair, and armor,
    effect and coordinates HUDs. One "key:value" per line, for example
    hitboxes:true or hitPrediction:false.

  Mesh shaders: OFF by default
    They were slower than the default multi-draw path on tested GPUs. To try
    them anyway, set the environment variable HORIZON_MESH_SHADERS=1 (0 forces
    them off).

  Rendering
    GPU culling, occlusion culling, arena-paged geometry with indirect
    multi-draw, TAA/FXAA/SMAA/CMAA2/FSR. Input lag and frame time come first.

Verify your download
  SHA256SUMS lists this folder's files; compare with:
    Get-FileHash horizon-client.exe -Algorithm SHA256

Source and releases: https://github.com/Louchatfroff/Perforizon
