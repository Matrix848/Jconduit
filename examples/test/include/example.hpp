#pragma once
#include <stdint.h>
#include <stdbool.h>

// ============================================================================
// 1. Enums (Tests Enum Detection & Backing Types Mapping)
// ============================================================================
typedef enum JcdTestStatus {
    JCD_STATUS_IDLE = 0,
    JCD_STATUS_INITIALIZING = 1,
    JCD_STATUS_RUNNING = 2,
    JCD_STATUS_PAUSED = 3,
    JCD_STATUS_ERROR = -1
} JcdTestStatus;

typedef enum JcdLogLevel {
    JCD_LOG_INFO = 10,
    JCD_LOG_WARN = 20,
    JCD_LOG_FAIL = 30
} JcdLogLevel;

// ============================================================================
// 2. Structs (Tests Padding, Offsets, and Memory Layout Generation)
// ============================================================================
struct JcdFlatConfig {
    uint32_t config_id;
    float global_scale;
    bool feature_enabled;
};

struct alignas(16) JcdAlignedVector {
    float x;
    float y;
    float z;
    float w;
};

// Nested Struct (Perfect for testing your recursive `flatten_type` loop!)
struct JcdTelemetryBatch {
    JcdFlatConfig config;      // Test: Nested Struct expansion
    JcdTestStatus state;       // Test: Enum translation inside struct
    uint64_t packet_index;
};

// Struct with Fixed Arrays
struct JcdRenderBounds {
    float min_max[4];          // Test: IrTypeKind::FixedArray handling
};

// Vector struct
struct JcdDynVec {
    JcdAlignedVector vec[];
};

// ============================================================================
// 3. Deferred Functions (Tests Queue Serialization & Param Flattening)
// ============================================================================

// Test Case: Primitive boundary arguments
[[jcd::deferred]] void jcd_cmd_submit_raw(uint32_t channel, int64_t value, float weight);

// Test Case: Enums passed directly as arguments
[[jcd::deferred]] void jcd_cmd_set_system_state(JcdTestStatus status, JcdLogLevel log_level);

// Test Case: Pass struct by value (Forces your generator to flatten fields into individual parameters)
[[jcd::deferred]] void jcd_cmd_update_config(JcdFlatConfig config);

// Test Case: Multi-layered deeply nested flattening (Stresses build_write_nodes offset logic)
[[jcd::deferred]] void jcd_cmd_push_telemetry(JcdTelemetryBatch batch);

// Test Case: Zero-parameter function (Ensures your generator skips creating empty payload structs!)
[[jcd::deferred]] void jcd_cmd_system_purge(void);


// ============================================================================
// 4. Direct Functions (Tests Immediate Invocation, Return Types & Out Pointers)
// ============================================================================

// Test Case: Direct primitive return path
float jcd_sys_get_performance_index(uint32_t worker_id);
bool jcd_sys_is_channel_active(uint32_t channel);

// Test Case: Enum returning direct function
JcdTestStatus jcd_sys_verify_current_status(void);

// Test Case: Pointer return path (Tests your pointer-reinterpretation and NULL checks)
const JcdTelemetryBatch* jcd_sys_peek_last_batch(void);

// Test Case: Out pointer mutation via annotations
void jcd_sys_fetch_config(uint32_t config_id, [[jcd::out]] JcdFlatConfig *const out_config);

// Test Case: Direct extraction of enum value via Out Pointer
void jcd_sys_capture_status([[jcd::out]] JcdTestStatus *const type_out);

// Test Case: Out Pointer tracking a complex aligned block
void jcd_sys_calculate_vector([[jcd::out]] JcdAlignedVector *const output);