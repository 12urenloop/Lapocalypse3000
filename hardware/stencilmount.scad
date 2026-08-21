//
// PCB + Solder Stencil Mount
//
// Coordinate system:
//   X = width
//   Y = height
//   Z = height
//
// The PCB sits in a recessed pocket.
// The stencil sits above the PCB and is constrained by the outer frame.
//

// ============================================================
// PCB DIMENSIONS
// ============================================================

pcb_width  = 50.0;       // PCB X dimension
pcb_height = 30.0;       // PCB Y dimension
pcb_thickness = 1.6;


// ============================================================
// STENCIL DIMENSIONS
// ============================================================

// Actual stencil dimensions
stencil_width  = 54.0;
stencil_height = 34.0;
stencil_thickness = 0.15;


// ============================================================
// PCB POSITION RELATIVE TO STENCIL
// ============================================================

// Positive values move the PCB relative to the stencil.
//
// Example:
//   pcb_offset_x = 0
//   pcb_offset_y = 0
//
// centers the PCB inside the stencil.
//
// These are particularly useful if the stencil is intentionally
// larger than the PCB.

pcb_offset_x = 0.0;
pcb_offset_y = 0.0;


// ============================================================
// STENCIL CLEARANCE
// ============================================================

// Clearance between stencil and the inside of the retaining
// frame.
//
// Smaller = tighter stencil fit.
// Larger = easier insertion/removal.

stencil_clearance_x = 0.25;
stencil_clearance_y = 0.25;


// ============================================================
// FRAME
// ============================================================

// Width of material surrounding the stencil.

frame_border = 8.0;


// ============================================================
// PCB POCKET CLEARANCE
// ============================================================

// Clearance around PCB.
//
// Keep this small enough that the PCB cannot move significantly,
// but large enough for easy insertion.

pcb_clearance_x = 0.10;
pcb_clearance_y = 0.10;


// ============================================================
// Z HEIGHTS
// ============================================================

// Thickness of the mounting plate.

base_thickness = 3.0;


// Depth of PCB pocket measured from the top surface.
//
// For example, with a 3 mm base and a 1.6 mm PCB:
// pcb_pocket_depth = 1.8 means the PCB top is 1.2 mm
// below the top surface.

pcb_pocket_depth = 1.8;


// Depth of the stencil recess.
//
// This should normally be approximately equal to the stencil
// thickness, or slightly deeper.

stencil_pocket_depth = 0.25;


// ============================================================
// OPTIONAL PCB SUPPORT
// ============================================================

// Extra material underneath PCB.
//
// This effectively determines how deep the PCB sits.

pcb_bottom_support = 0.5;


// ============================================================
// CORNER / EDGE OPTIONS
// ============================================================

// Radius of outer frame corners.
// Set to 0 for square corners.

outer_corner_radius = 2;


// Radius of PCB pocket corners.
// Usually 0 is preferable for rectangular PCBs.

pcb_corner_radius = 0;


// ============================================================
// CALCULATED DIMENSIONS
// ============================================================

// Stencil opening/frame size
stencil_outer_width =
    stencil_width + 2 * stencil_clearance_x;

stencil_outer_height =
    stencil_height + 2 * stencil_clearance_y;


// Overall mount size
mount_width =
    stencil_outer_width + 2 * frame_border;

mount_height =
    stencil_outer_height + 2 * frame_border;


// PCB pocket dimensions
pcb_pocket_width =
    pcb_width + 2 * pcb_clearance_x;

pcb_pocket_height =
    pcb_height + 2 * pcb_clearance_y;


// PCB position in mount
//
// First center the stencil in the mount,
// then offset the PCB relative to the stencil.

pcb_center_x =
    mount_width / 2
    + pcb_offset_x;

pcb_center_y =
    mount_height / 2
    + pcb_offset_y;


// ============================================================
// HELPER: ROUNDED RECTANGLE
// ============================================================

module rounded_rectangle(width, height, radius) {

    if (radius <= 0) {
        square([width, height], center=true);
    }
    else {
        offset(r=radius)
            square([
                width - 2 * radius,
                height - 2 * radius
            ], center=true);
    }
}


// ============================================================
// MAIN MOUNT
// ============================================================

difference() {

    // --------------------------------------------------------
    // SOLID OUTER BODY
    // --------------------------------------------------------

    translate([
        mount_width / 2,
        mount_height / 2,
        0
    ])
    linear_extrude(height=base_thickness)
        rounded_rectangle(
            mount_width,
            mount_height,
            outer_corner_radius
        );


    // --------------------------------------------------------
    // STENCIL RECESS
    // --------------------------------------------------------
    //
    // This creates a shallow pocket corresponding to the
    // stencil dimensions.
    //
    // The stencil is constrained laterally by the remaining
    // frame.
    //

    translate([
        mount_width / 2,
        mount_height / 2,
        base_thickness - stencil_pocket_depth
    ])
    linear_extrude(
        height=stencil_pocket_depth + 0.1
    )
    square([
        stencil_outer_width,
        stencil_outer_height
    ], center=true);


    // --------------------------------------------------------
    // PCB POCKET
    // --------------------------------------------------------
    //
    // The PCB pocket is deeper than the stencil pocket.
    //

    translate([
        pcb_center_x,
        pcb_center_y,
        base_thickness - stencil_pocket_depth - pcb_pocket_depth - pcb_thickness
    ])
    linear_extrude(
        height=pcb_pocket_depth + pcb_thickness + 0.1
    )
    rounded_rectangle(
        pcb_pocket_width,
        pcb_pocket_height,
        pcb_corner_radius
    );
}