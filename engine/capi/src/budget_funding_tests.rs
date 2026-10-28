// SPDX-License-Identifier: GPL-3.0-or-later
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors

use super::*;

#[test]
fn local_funding_changes_use_float32_percent_scale() {
    let g = Game::new();
    unsafe {
        let (_, bh) = g.create(CLSID_BUDGET);
        let b = bh as *mut fc4_budget;
        assert!(fc4_budget_init(b, 0, false));
        let dt = fc4_budget_create_department(b, 4096, 8192);
        let d = g.dept(dt);
        assert_ne!(fc4_department_create_line_item(d, 2, true), 0);
        let (ht, _) = g.create(HOLDER_CLASS);
        set_u32s(g.host().user, ht, 0xEA54_D283, [4096u32].as_ptr(), 1);
        set_u32s(g.host().user, ht, 0xEA54_D284, [2u32].as_ptr(), 1);
        set_u32s(g.host().user, ht, 0xEA54_D285, [7u32].as_ptr(), 1);
        set_i64s(g.host().user, ht, 0xEA54_D286, [300i64].as_ptr(), 1);
        assert!(fc4_department_add_locally_funded_object(d, ht, 7));

        // Budget sections 7.3/7.4 and foundations 18.3: 20 percent is
        // 20 * 0.01f = 0x3E4CCCCC, one ULP below the float32 literal 0.2.
        assert!(fc4_department_set_local_funding_percent(d, ht, 0.2, 7));
        assert_eq!(fc4_department_get_funding_percentage(d).to_bits(), 0x3E4C_CCCC);
        assert!(fc4_department_set_local_full_funding(d, ht, 400, 7));
        assert_eq!(fc4_department_get_funding_percentage(d).to_bits(), 0x3E4C_CCCC);
    }
}
