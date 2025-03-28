# Input Handling

Terrarium only forwards input events from winit and openxr. It leaves it up to you how to handle these input events. This example tries to demonstrate how this can be done gracefully.

The available properties in `XrHandState` vary based on the connected XR device, or will be left completely empty when testing in a disconnected environment.

All available control paths can be found at the [OpenXR Specification](https://www.khronos.org/registry/OpenXR/specs/1.0/html/xrspec.html).

## Khronos Simple Controller Profile
These controls are guaranteed to be available on any XR device.

| OpenXR Path                        | Type     |
| ---------------------------------- | -------- |
| `/input/select/click`              | Digital  |
| `/input/menu/click`                | Digital  |
| `/input/grip/pose`                 | Pose     |
| `/input/aim/pose`                  | Pose     |

## Oculus Touch Controller Profile

| OpenXR Path                        | Type     |
| ---------------------------------- | -------- |
| `/input/thumbstick`                | Analog2D |
| `/input/squeeze/value`             | Analog   |
| `/input/menu/click`                | Digital  |
| `/input/system/click`              | Digital  |
| `/input/a/click`                   | Digital  |
| `/input/a/touch`                   | Digital  |
| `/input/b/click`                   | Digital  |
| `/input/b/touch`                   | Digital  |
| `/input/x/click`                   | Digital  |
| `/input/x/touch`                   | Digital  |
| `/input/y/click`                   | Digital  |
| `/input/y/touch`                   | Digital  |
| `/input/trigger/value`             | Analog   |
| `/input/trigger/touch`             | Digital  |
| `/input/thumbstick/click`          | Digital  |
| `/input/thumbstick/touch`          | Digital  |
| `/input/grip/pose`                 | Pose     |
| `/input/aim/pose`                  | Pose     |