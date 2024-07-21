Stm32Loader
=

This is a commandline tool to upload firmware to an STM32 mcu. 
Currently, in a very early prototype phase. Good enough to upload
firmware using the SPI protocol on an STM32F401RE from an Raspberry PI.

For reference see:
https://www.st.com/resource/en/application_note/an2606-stm32-microcontroller-system-memory-boot-mode-stmicroelectronics.pdf
https://www.st.com/resource/en/application_note/an4286-spi-protocol-used-in-the-stm32-bootloader-stmicroelectronics.pdf

```
Usage: Stm32Loader [OPTIONS] <COMMAND>

Commands:
  read
  write
  unprotect
  erase-all
  go
  help       Print this message or the help of the given subcommand(s)

Options:
      --type <PORTTYPE>  Select the bootloader interface: Serial, SPI or I2C
      --port <PORTNAME>  The name of a device port, e.g. spidev0.1
  -h, --help             Print help
  -V, --version          Print version
```

