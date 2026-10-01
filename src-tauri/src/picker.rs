//! Quick Picker 的定位几何：光标四象限展开 + 屏幕边界 clamp。
//!
//! Seam：`compute_anchor_position`（纯函数，物理像素进出）。
//! 多显示器：调用方先用 `monitor_from_point` 取光标所在显示器，本函数只在
//! 该显示器矩形内摆放，天然不会跳屏。

/// 呼出窗口与光标的间距（物理像素）。
pub const PICKER_MARGIN: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 光标四象限展开：水平/垂直各朝剩余空间更大的一侧展开
/// （严格大于半宽/半高才视为「空间更大」），再 clamp 进显示器矩形；
/// 窗口大于显示器时贴住左上角。返回窗口左上角坐标。
pub fn compute_anchor_position(
    cursor: (f64, f64),
    monitor: Rect,
    window: (f64, f64),
    margin: f64,
) -> (f64, f64) {
    let (cx, cy) = cursor;
    let (w, h) = window;
    let left_space = cx - monitor.x;
    let top_space = cy - monitor.y;

    let x = if left_space > monitor.width / 2.0 {
        cx - w - margin
    } else {
        cx + margin
    };
    let y = if top_space > monitor.height / 2.0 {
        cy - h - margin
    } else {
        cy + margin
    };

    let min_x = monitor.x;
    let max_x = (monitor.x + monitor.width - w).max(min_x);
    let min_y = monitor.y;
    let max_y = (monitor.y + monitor.height - h).max(min_y);
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1920x1080 主屏，640x420 呼出窗，边距 12。
    const MONITOR: Rect = Rect { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0 };
    const WIN: (f64, f64) = (640.0, 420.0);
    const MARGIN: f64 = 12.0;

    #[test]
    fn cursor_center_opens_right_and_below() {
        // 左侧空间 = 半宽 → 视为右侧展开；垂直同理
        let (x, y) = compute_anchor_position((960.0, 540.0), MONITOR, WIN, MARGIN);
        assert_eq!((x, y), (972.0, 552.0));
    }

    #[test]
    fn cursor_bottom_right_opens_left_and_above() {
        let (x, y) = compute_anchor_position((1800.0, 1000.0), MONITOR, WIN, MARGIN);
        assert_eq!((x, y), (1148.0, 568.0));
    }

    #[test]
    fn cursor_near_edges_clamps_into_monitor() {
        // 右下角：先四象限展开，再被 clamp 回屏幕内
        let (x, y) = compute_anchor_position((1910.0, 1070.0), MONITOR, WIN, MARGIN);
        assert_eq!((x, y), (1258.0, 638.0));
        assert!(x + WIN.0 <= MONITOR.width && y + WIN.1 <= MONITOR.height);

        // 小屏：下方空间不足 → clamp 底边
        let small = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
        let (x, y) = compute_anchor_position((700.0, 300.0), small, WIN, MARGIN);
        assert_eq!((x, y), (48.0, 180.0));
        assert!(y + WIN.1 <= small.height);
    }

    #[test]
    fn window_larger_than_monitor_pins_to_origin() {
        let tiny = Rect { x: 0.0, y: 0.0, width: 600.0, height: 400.0 };
        let (x, y) = compute_anchor_position((300.0, 200.0), tiny, WIN, MARGIN);
        assert_eq!((x, y), (0.0, 0.0));
    }

    #[test]
    fn second_monitor_keeps_position_relative_to_itself() {
        // 多显示器：右屏 (1920,0,1920x1080)，光标贴其左缘 → 右侧展开且不出屏
        let right = Rect { x: 1920.0, y: 0.0, width: 1920.0, height: 1080.0 };
        let (x, y) = compute_anchor_position((1930.0, 500.0), right, WIN, MARGIN);
        assert_eq!((x, y), (1942.0, 512.0));
        assert!(x >= right.x && x + WIN.0 <= right.x + right.width);
    }
}
