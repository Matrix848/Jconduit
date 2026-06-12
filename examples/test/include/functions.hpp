#pragma once
#include "typedef.hpp"

// void functions
void body_destroy(uint32_t space_id, uint64_t body_id);
void body_set_position(uint32_t space_id, uint64_t body_id, Vector3f pos);
void body_set_type(uint32_t space_id, uint64_t body_id, BodyType body_type);
void space_reset(uint32_t space_id);
void engine_shutdown(void);

// typed functions (return values)
const Vector3f* body_get_position(uint32_t space_id, uint64_t body_id);
void body_get_rotation(uint32_t space_id,  uint64_t body_id, Quaternion const *rot_out);
void query_closest_contact(uint32_t space_id, uint64_t body_a, uint64_t body_b, Contact *const rot_out);
void body_get_normal(uint32_t space_id, uint64_t body_id, Vector3f *const output); // Won't generate scratch override
void body_get_type(uint32_t space_id, uint64_t body_id, BodyType *const type_out);
float body_get_mass(uint32_t space_id, uint64_t body_id);
bool body_is_sleeping(uint32_t space_id, uint64_t body_id);

// Will rightfully panic, you should never return by value.
// Vector3f body_get_velocity(uint32_t space_id, uint64_t body_id);
// Will panic: you _out parameter should be last.
// void body_get_type(uint32_t space_id, BodyType *const type_out, uint64_t body_id);