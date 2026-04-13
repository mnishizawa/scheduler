# schedule - Makefile

# Default target: build the project
.PHONY: build
build:
	cargo build

# Run the scheduler with the default config (~/.config/schedule/config.toml)
.PHONY: run
run:
	cargo run

# Dynamic testing target:
# 1. Calculates a time 1 minute into the future (using macOS 'date -v').
# 2. Generates a temporary TOML configuration.
# 3. Executes 'cargo run' with the temporary file.
# 4. Cleans up the temporary file automatically.
.PHONY: test-run
test-run:
	@TARGET_TIME=$$(date -v+1M +"%H:%M"); \
	TEMP_FILE=$$(mktemp /tmp/schedule-test.XXXXXX); \
	trap "rm -f $$TEMP_FILE" EXIT INT TERM; \
	echo "[[job]]" > $$TEMP_FILE; \
	echo "name     = \"Dynamic Test Job (at $$TARGET_TIME)\"" >> $$TEMP_FILE; \
	echo "schedule = \"every day at $$TARGET_TIME\"" >> $$TEMP_FILE; \
	echo "command  = \"echo\"" >> $$TEMP_FILE; \
	echo "args     = [\"✅ Testing... Success!\"]" >> $$TEMP_FILE; \
	echo "------------------------------------------------------------"; \
	echo "🚀 Starting test run..."; \
	echo "⏳ Job scheduled for: $$TARGET_TIME (in ~1 minute)"; \
	echo "📄 Using temporary config: $$TEMP_FILE"; \
	echo "------------------------------------------------------------"; \
	cargo run -- $$TEMP_FILE; \
	echo "------------------------------------------------------------"; \
	echo "🧹 Cleaning up temporary files...";
