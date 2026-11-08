module;

export module firmware;

export import anchor;

export void firmware_setup()
{
    anchor_setup();
}

export void firmware_loop()
{
    anchor_loop();
}
