module;

export module firmware;

export import gateway;

export void firmware_setup()
{
    gateway_setup();
}

export void firmware_loop()
{
    gateway_loop();
}
