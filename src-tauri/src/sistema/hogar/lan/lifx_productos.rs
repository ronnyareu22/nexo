//! Qué sabe hacer cada luz LIFX por su número de producto (github.com/LIFX/products, sin las mejoras por
//! versión). Generado por `scripts/tablas-red.py`: no editar a mano.

/// (producto, color, Kelvin mínimo, Kelvin máximo (0 si no cambia de blanco), interruptor de relés).
pub const PRODUCTOS: [(u16, bool, u16, u16, bool); 173] = [
    (1, true, 2500, 9000, false), // Original
    (2, true, 2500, 9000, false), // Original 1000
    (3, true, 2500, 9000, false), // Color 650
    (10, false, 2700, 6500, false), // LIFX White 800 (LV)
    (11, false, 2700, 6500, false), // LIFX White 800 (HV)
    (12, false, 2700, 2700, false), // LIFX LCMv4 White OEM (A19 LV)
    (13, false, 2700, 2700, false), // LIFX LCMv4 White OEM (A19 HV)
    (14, true, 2500, 9000, false), // LIFX Carbon Prototype
    (15, true, 2700, 6500, false), // LIFX LCMv4 Color (A21)
    (18, false, 2700, 6500, false), // LIFX White 900 (BR30)
    (19, false, 2700, 6500, false), // LIFX White 900 (BR30)
    (20, true, 2500, 9000, false), // LIFX Color 1000 (BR30)
    (21, true, 2500, 9000, false), // LIFX LCMv4 Color (GU10)
    (22, true, 2500, 9000, false), // LIFX Color 1000
    (23, true, 2500, 9000, false), // LIFX (A19)
    (24, true, 2500, 9000, false), // LIFX (BR30)
    (25, true, 2500, 9000, false), // LIFX+ (A19)
    (26, true, 2500, 9000, false), // LIFX+ (BR30)
    (27, true, 2500, 9000, false), // LIFX (A19)
    (28, true, 2500, 9000, false), // LIFX (BR30)
    (29, true, 2500, 9000, false), // LIFX+ (A19)
    (30, true, 2500, 9000, false), // LIFX+ (BR30)
    (31, true, 2500, 9000, false), // LIFX Z
    (32, true, 2500, 9000, false), // LIFX Z
    (33, true, 2500, 9000, false), // LIFX Downlight
    (36, true, 2500, 9000, false), // LIFX DL
    (37, true, 2500, 9000, false), // LIFX DL
    (38, true, 2500, 9000, false), // LIFX Beam
    (39, false, 2500, 9000, false), // LIFX DL WW 700lm
    (40, true, 2500, 9000, false), // LIFX DL Colour 700lm
    (43, true, 2500, 9000, false), // LIFX (A19)
    (44, true, 2500, 9000, false), // LIFX (BR30)
    (45, true, 2500, 9000, false), // LIFX+ (A19)
    (46, true, 2500, 9000, false), // LIFX+ (BR30)
    (49, true, 2500, 9000, false), // LIFX Mini C
    (50, false, 1500, 4000, false), // LIFX Mini DD
    (51, false, 2700, 2700, false), // LIFX Mini W
    (52, true, 2500, 9000, false), // LIFX GU10
    (53, true, 2500, 9000, false), // LIFX GU10
    (54, false, 2700, 2700, false), // LIFX GU10 White
    (55, true, 2500, 9000, false), // LIFX Tile
    (56, true, 2500, 9000, false), // LIFX Beam
    (57, true, 2500, 9000, false), // LIFX Candle C
    (59, true, 2500, 9000, false), // LIFX Mini C
    (60, false, 1500, 4000, false), // LIFX Mini DD
    (61, false, 2700, 2700, false), // LIFX Mini W
    (62, true, 2500, 9000, false), // LIFX (A19) LCM3
    (63, true, 2500, 9000, false), // LIFX (BR30) LCM3
    (64, true, 2500, 9000, false), // LIFX+ (A19) LCM3
    (65, true, 2500, 9000, false), // LIFX+ (BR30) LCM3
    (66, false, 2700, 2700, false), // LIFX Mini W
    (67, false, 2200, 6500, false), // LIFX Candle WW
    (68, true, 2500, 9000, false), // LIFX Candle C
    (70, false, 0, 0, true), // LIFX Switch
    (71, false, 0, 0, true), // LIFX Switch
    (72, true, 1500, 9000, false), // LIFX A19
    (81, false, 2200, 6500, false), // LIFX Candle WW
    (82, false, 2200, 2200, false), // LIFX Filament Clear
    (84, false, 0, 0, true), // LIFX Switch
    (85, false, 2000, 2000, false), // LIFX Filament Amber
    (86, false, 2200, 2200, false), // LIFX Filament Smokey
    (87, false, 2700, 2700, false), // LIFX White
    (88, false, 2700, 2700, false), // LIFX White
    (89, false, 0, 0, true), // LIFX Switch
    (90, true, 2500, 9000, false), // LIFX Clean A19 1100lm
    (91, true, 2500, 9000, false), // LIFX Color 800lm
    (92, true, 2500, 9000, false), // LIFX Colour 1000lm
    (93, true, 2500, 9000, false), // LIFX Color A19 1100lm
    (94, true, 2500, 9000, false), // LIFX Color BR30 1100lm
    (96, false, 2200, 6500, false), // LIFX Candle WW
    (97, true, 2500, 9000, false), // LIFX Colour A19 1200lm
    (98, true, 2500, 9000, false), // LIFX Colour BR30 1100lm
    (99, true, 2500, 9000, false), // LIFX Clean A19 1200lm
    (100, false, 2200, 2200, false), // LIFX Filament Clear
    (101, false, 2000, 2000, false), // LIFX Filament Amber
    (102, false, 2200, 2200, false), // LIFX Filament Smokey
    (109, true, 2500, 9000, false), // LIFX NV A19 1100lm
    (110, true, 2500, 9000, false), // LIFX NV BR30 1100lm
    (111, true, 2500, 9000, false), // LIFX NV A19 1200lm
    (112, true, 2500, 9000, false), // LIFX NV BR30 1100lm
    (113, false, 1500, 4000, false), // LIFX WW 800lm
    (114, false, 1500, 4000, false), // LIFX WW 1000lm
    (115, false, 0, 0, true), // LIFX Switch
    (116, false, 0, 0, true), // LIFX Switch US
    (117, true, 1500, 9000, false), // LIFX Z
    (118, true, 1500, 9000, false), // LIFX Z
    (119, true, 1500, 9000, false), // LIFX Beam
    (120, true, 1500, 9000, false), // LIFX Beam
    (121, true, 1500, 9000, false), // LIFX DL
    (122, true, 1500, 9000, false), // LIFX DL
    (123, true, 1500, 9000, false), // LIFX Mini 3.1 Color US
    (124, true, 1500, 9000, false), // LIFX Mini 3.1 Color Intl
    (125, false, 1500, 9000, false), // LIFX Mini 3.1 WW US
    (126, false, 1500, 9000, false), // LIFX Mini 3.1 WW Intl
    (127, false, 2700, 2700, false), // LIFX Mini 3.1 W US
    (128, false, 2700, 2700, false), // LIFX Mini 3.1 W Intl
    (129, true, 1500, 9000, false), // LIFX Color 800
    (130, true, 1500, 9000, false), // LIFX Colour 1000
    (131, false, 1500, 9000, false), // LIFX White to Warm 800
    (132, false, 1500, 9000, false), // LIFX White to Warm 1000
    (133, false, 2700, 2700, false), // LIFX White 650
    (134, false, 2700, 2700, false), // LIFX White 800
    (135, true, 1500, 9000, false), // LIFX GU10 Color US
    (136, true, 1500, 9000, false), // LIFX GU10 Color Intl
    (137, true, 1500, 9000, false), // LIFX Candle C
    (138, true, 1500, 9000, false), // LIFX Candle C
    (141, true, 1500, 9000, false), // LIFX Neon
    (142, true, 1500, 9000, false), // LIFX Neon
    (143, true, 1500, 9000, false), // LIFX String
    (144, true, 1500, 9000, false), // LIFX String
    (151, true, 1500, 9000, false), // LIFX Neon Outdoor
    (152, true, 1500, 9000, false), // LIFX Neon Outdoor
    (153, true, 1500, 9000, false), // LIFX PAR38 US
    (154, true, 1500, 9000, false), // LIFX PAR38 Intl
    (155, true, 1500, 9000, false), // LIFX Sunshine
    (156, true, 1500, 9000, false), // LIFX A21 1000lm Intl
    (161, true, 1500, 9000, false), // LIFX Neon Outdoor
    (162, true, 1500, 9000, false), // LIFX Neon Outdoor
    (163, true, 1500, 9000, false), // LIFX A19
    (164, true, 1500, 9000, false), // LIFX BR30
    (165, true, 1500, 9000, false), // LIFX A19 Intl
    (166, true, 1500, 9000, false), // LIFX BR30 Intl
    (167, true, 1500, 9000, false), // LIFX DL AU
    (168, true, 1500, 9000, false), // LIFX DL US
    (169, true, 1500, 9000, false), // LIFX A21
    (170, true, 1500, 9000, false), // LIFX A21
    (171, true, 1500, 9000, false), // LIFX Spot
    (172, true, 1500, 9000, false), // LIFX Spot
    (173, true, 1500, 9000, false), // LIFX Path
    (174, true, 1500, 9000, false), // LIFX Path
    (175, true, 1500, 9000, false), // LIFX PAR38
    (176, true, 1500, 9000, false), // LIFX Ceiling
    (177, true, 1500, 9000, false), // LIFX Ceiling
    (178, true, 1500, 9000, false), // LIFX Downlight US
    (179, true, 1500, 9000, false), // LIFX Downlight US
    (180, true, 1500, 9000, false), // LIFX Downlight US
    (181, true, 1500, 9000, false), // LIFX Mini
    (182, true, 1500, 9000, false), // LIFX Mini
    (185, true, 1500, 9000, false), // LIFX Candle C
    (186, true, 1500, 9000, false), // LIFX Candle C Intl
    (187, true, 1500, 9000, false), // LIFX Candle
    (188, true, 1500, 9000, false), // LIFX Candle Intl
    (191, true, 2700, 6500, false), // LIFX Everyday A19
    (192, true, 2700, 6500, false), // LIFX Everyday A19 Intl
    (201, true, 1500, 9000, false), // LIFX Ceiling 13x26
    (202, true, 1500, 9000, false), // LIFX Ceiling 13x26 Intl
    (203, true, 1500, 9000, false), // LIFX String
    (204, true, 1500, 9000, false), // LIFX String
    (205, true, 1500, 9000, false), // LIFX Neon
    (206, true, 1500, 9000, false), // LIFX Neon
    (207, true, 2500, 6500, false), // LIFX Everyday Lightstrip
    (208, true, 2500, 6500, false), // LIFX Everyday Lightstrip Intl
    (211, true, 2700, 6500, false), // LIFX Everyday Permanent Outdoor US
    (213, true, 1500, 9000, false), // LIFX Permanent Outdoor
    (214, true, 1500, 9000, false), // LIFX Permanent Outdoor Intl
    (215, true, 1500, 9000, false), // LIFX Candle C
    (216, true, 1500, 9000, false), // LIFX Candle C Intl
    (217, true, 1500, 9000, false), // LIFX Tube
    (218, true, 1500, 9000, false), // LIFX Tube Intl
    (219, true, 1500, 9000, false), // LIFX Luna
    (220, true, 1500, 9000, false), // LIFX Luna Intl
    (221, true, 1500, 9000, false), // LIFX Spot Intl
    (222, true, 1500, 9000, false), // LIFX Path Intl
    (223, true, 1500, 9000, false), // LIFX DL
    (224, true, 1500, 9000, false), // LIFX DL Intl
    (225, true, 1500, 9000, false), // LIFX PAR38 Intl
    (226, false, 0, 0, true), // LIFX Dimmer Switch
    (229, true, 1000, 10000, false), // LIFX Path Intl
    (265, true, 1500, 9000, false), // LIFX Ceiling 13
    (266, true, 1500, 9000, false), // LIFX Ceiling 13 Intl
    (267, true, 1500, 9000, false), // LIFX Mirror
    (268, true, 1500, 9000, false), // LIFX Mirror Intl
    (300, true, 2700, 6500, false), // LIFX Everyday Permanent Outdoor US
];
