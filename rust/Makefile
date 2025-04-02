# Top-level Makefile for WOFS Rust kernel module

# Default kernel build directory
KDIR ?= /lib/modules/$(shell uname -r)/build

# Installation directory
INSTALL_MOD_DIR ?= extra

# Default target
all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

# Clean target
clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean

# Installation target
install:
	$(MAKE) -C $(KDIR) M=$(PWD) modules_install INSTALL_MOD_DIR=$(INSTALL_MOD_DIR)

.PHONY: all clean install
