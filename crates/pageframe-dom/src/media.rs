use std::sync::Arc;

use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;

use crate::BaseDocument;
use crate::layout::damage::ALL_DAMAGE;
use crate::node::{ImageData, RasterImageData, SpecialElementData};

pub fn present_frame(
    doc: &mut BaseDocument,
    node_id: NodeId,
    width: u32,
    height: u32,
    rgba: Arc<Vec<u8>>,
) {
    let Some(node) = doc.get_node_mut(node_id) else {
        return;
    };
    let Some(element) = node.element_data_mut() else {
        return;
    };
    if element.name.local != local_name!("video") || width == 0 || height == 0 {
        return;
    }
    let same_size = element
        .raster_image_data()
        .is_some_and(|data| data.width == width && data.height == height);
    element.special_data = SpecialElementData::Image(Box::new(ImageData::Raster(
        RasterImageData::new(width, height, rgba),
    )));
    if !same_size {
        node.clear_layout_cache();
        node.insert_damage(ALL_DAMAGE);
    }
}

pub fn clear_frame(doc: &mut BaseDocument, node_id: NodeId) {
    let Some(node) = doc.get_node_mut(node_id) else {
        return;
    };
    let Some(element) = node.element_data_mut() else {
        return;
    };
    if element.name.local != local_name!("video") || element.raster_image_data().is_none() {
        return;
    }
    element.special_data = SpecialElementData::None;
    node.clear_layout_cache();
    node.insert_damage(ALL_DAMAGE);
}
