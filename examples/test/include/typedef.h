#pragma once
#include <stdint.h>
#include <stdbool.h>

typedef struct {
    float x;
    float y;
    float z;
} Vector3f;

typedef struct __attribute__((aligned(16))) {
    float x;
    float y;
    float z;
    float w;
} Quaternion;

typedef struct {
    Vector3f position;
    Vector3f normal;
    float depth;
} Contact;

typedef enum {
    BODY_TYPE_STATIC = 0,
    BODY_TYPE_DYNAMIC = 1,
    BODY_TYPE_KINEMATIC = 2,
} BodyType;