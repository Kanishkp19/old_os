################################################################################
# Home Hub Rust binaries from the checked-out workspace
################################################################################

HOMEHUB_VERSION = local
HOMEHUB_SITE = $(BR2_EXTERNAL_HOMEHUB_PATH)/../../hub
HOMEHUB_SITE_METHOD = local
HOMEHUB_LICENSE = Proprietary
HOMEHUB_DEPENDENCIES = host-rustc openssl sqlite

define HOMEHUB_BUILD_CMDS
	cd $(@D) && $(TARGET_MAKE_ENV) \
		CARGO_TARGET_DIR=$(@D)/target \
		CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$(TARGET_CC)" \
		PKG_CONFIG_ALLOW_CROSS=1 \
		PKG_CONFIG_SYSROOT_DIR="$(STAGING_DIR)" \
		$(HOST_DIR)/bin/cargo build --locked --release \
		--target $(RUSTC_TARGET_NAME) \
		-p hh-service -p hh-session -p hh-tools
endef

define HOMEHUB_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/hh-service \
		$(TARGET_DIR)/usr/bin/hh-service
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/hh-session \
		$(TARGET_DIR)/usr/libexec/hh-session
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/hh-tools \
		$(TARGET_DIR)/usr/bin/hh-tools
endef

$(eval $(generic-package))
