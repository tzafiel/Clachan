#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderItem {
    SimEntity(usize),
    LdtkEntity(usize),
    Foliage(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct SortedRenderItem {
    pub foot_y: f32,
    pub order: usize,
    pub item: RenderItem,
}

pub fn sort_render_items(items: &mut [SortedRenderItem]) {
    items.sort_by(|left, right| {
        left.foot_y
            .total_cmp(&right.foot_y)
            .then_with(|| left.order.cmp(&right.order))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_by_world_foot_y_and_preserves_ties() {
        let mut items = [
            SortedRenderItem {
                foot_y: 4.0,
                order: 0,
                item: RenderItem::SimEntity(0),
            },
            SortedRenderItem {
                foot_y: 1.0,
                order: 2,
                item: RenderItem::SimEntity(2),
            },
            SortedRenderItem {
                foot_y: 1.0,
                order: 1,
                item: RenderItem::LdtkEntity(1),
            },
        ];

        sort_render_items(&mut items);
        assert_eq!(items[0].item, RenderItem::LdtkEntity(1));
        assert_eq!(items[1].item, RenderItem::SimEntity(2));
        assert_eq!(items[2].item, RenderItem::SimEntity(0));
    }
}
