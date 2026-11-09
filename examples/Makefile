# Atari 7800 Examples Top-Level Makefile
# Builds all sample ROMs and demos across the examples directory.

CARGO ?= cargo

# List of all sample projects
SAMPLES := \
	pixel \
	static_sprite \
	transform_sprite \
	scale_sprite \
	bugs \
	bugs-lokey

.PHONY: all clean tools help $(SAMPLES) $(addprefix clean-,$(SAMPLES))

# Default target: build workspace tools, then build all sample ROMs
all: tools $(SAMPLES)

# Build host workspace CLI tools (a78tool, a78gfx, a78sign)
tools:
	@$(CARGO) build --manifest-path ../Cargo.toml --workspace

# Per-sample build targets
$(SAMPLES): tools
	@$(MAKE) -C $@

# Clean all samples
clean: $(addprefix clean-,$(SAMPLES))

# Per-sample clean targets
$(addprefix clean-,$(SAMPLES)):
	@$(MAKE) -C $(patsubst clean-%,%,$@) clean

# Help target listing available commands and samples
help:
	@echo "Atari 7800 Examples Build System"
	@echo ""
	@echo "Targets:"
	@echo "  all          Build all example ROMs (default)"
	@echo "  tools        Build workspace CLI tools (a78tool, a78gfx, a78sign)"
	@echo "  clean        Clean build artifacts in all example directories"
	@echo "  help         Display this help message"
	@echo ""
	@echo "Individual samples:"
	@for sample in $(SAMPLES); do \
		echo "  $$sample"; \
	done
	@echo ""
	@echo "Individual clean targets:"
	@for sample in $(SAMPLES); do \
		echo "  clean-$$sample"; \
	done
