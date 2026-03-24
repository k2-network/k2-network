use super::*;
use std::time::{Instant, Duration};
use futures::StreamExt;
use std::path::PathBuf;
use anyhow::Result;

/// Hàm hỗ trợ tạo thư mục tạm thời để kiểm thử việc chia sẻ tệp.
/// Thư mục này sẽ được đặt tên dựa trên ID ngẫu nhiên của K2Marketplace để tránh xung đột.
fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k2-test-{}", K2Marketplace::generate_id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn test_contact_book_docs_full_lifecycle() -> Result<()> {
    // 1. Khởi tạo một Node K2 mới và lấy trình quản lý danh bạ dựa trên iroh-docs
    let node = K2Node::new().await?;
    let mut cb = node.contact_book();
    cb.init().await?;

    let node_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
    let nickname = "Alice".to_string();

    // 2. Kiểm thử tính năng THÊM liên lạc
    cb.add(node_id.clone(), nickname.clone(), Some("Friend".to_string())).await?;
    
    // 3. Kiểm thử tính năng LẤY thông tin liên lạc theo Node ID
    let contact = cb.get(&node_id).await?.expect("Liên lạc phải tồn tại sau khi thêm");
    assert_eq!(contact.nickname, nickname);
    
    // 4. Kiểm thử tính năng CẬP NHẬT biệt danh (nickname)
    cb.update_nickname(&node_id, "Alice Updated".to_string()).await?;
    let contact = cb.get(&node_id).await?.expect("Liên lạc phải tồn tại sau khi cập nhật");
    assert_eq!(contact.nickname, "Alice Updated");

    // 5. Kiểm thử tính năng LIỆT KÊ danh sách liên lạc
    let list = cb.list().await?;
    assert!(list.iter().any(|c| c.node_id == node_id));

    // 6. Kiểm thử tính năng XÓA liên lạc
    let removed = cb.remove(&node_id).await?;
    assert!(removed);
    let contact = cb.get(&node_id).await?;
    assert!(contact.is_none());

    Ok(())
}

#[tokio::test]
async fn test_file_sharing_mechanics() -> Result<()> {
    // 1. Khởi tạo Node và dữ liệu mẫu để chia sẻ
    let node = K2Node::new().await?;
    let data = b"Hello K2 P2P World!";
    let filename = "test.txt";

    // 2. Kiểm thử chia sẻ dữ liệu dưới dạng byte và nhận vé (ticket)
    let ticket_str = node.share_bytes(data, filename).await?;
    assert!(ticket_str.starts_with(filename));

    // 3. Kiểm thử tải tệp về (Local loopback - tự tải từ chính mình)
    let save_dir = temp_dir();
    let downloaded_name = node.download_file(&ticket_str, &save_dir).await?;
    assert_eq!(downloaded_name, filename);

    // 4. Kiểm chứng tính toàn vẹn của dữ liệu sau khi tải về
    let downloaded_data = std::fs::read(save_dir.join(filename))?;
    assert_eq!(downloaded_data, data);

    // 5. Dọn dẹp thư mục tạm
    let _ = std::fs::remove_dir_all(save_dir);
    Ok(())
}

#[tokio::test]
async fn test_marketplace_message_integrity() -> Result<()> {
    // 1. Tạo một thông điệp chào bán (Offer) mẫu
    let offer = MarketplaceOffer {
        id: K2Marketplace::generate_id(),
        node_id: "test-node".to_string(),
        topic: "Digital Assets".to_string(),
        action: "Sell".to_string(),
        subtopic: None,
        category: Some("Tokens".to_string()),
        skill: None,
        title: "Bitcoin".to_string(),
        description: "Selling 1 BTC".to_string(),
        price_min: 50000,
        price_max: 55000,
        currency: "USD".to_string(),
        timestamp: 123456789,
    };

    let msg = MarketplaceMessage::Offer(offer.clone());
    
    // 2. Kiểm thử quá trình MÃ HÓA (Serialization) sang nhị phân
    let bytes = K2Marketplace::serialize_message(&msg)?;
    
    // 3. Kiểm thử quá trình GIẢI MÃ (Deserialization) ngược lại
    let decoded = K2Marketplace::deserialize_message(&bytes)?;
    
    // 4. So sánh dữ liệu gốc và dữ liệu sau khi giải mã
    if let MarketplaceMessage::Offer(decoded_offer) = decoded {
        assert_eq!(decoded_offer.id, offer.id);
        assert_eq!(decoded_offer.title, offer.title);
    } else {
        panic!("Loại thông điệp sau khi giải mã không khớp");
    }

    Ok(())
}

#[tokio::test]
async fn benchmark_multi_node_marketplace_flow() -> Result<()> {
    let topic_name = format!("market-bench-{}", iroh::SecretKey::generate(&mut rand::rng()).public());
    let topic_id = K2Marketplace::topic_to_id(&topic_name);

    println!("\n=== ĐO LƯỜNG CHU TRÌNH MARKETPLACE (3 NODES) ===");

    // Tạo 3 nodes
    let node1 = K2Node::new().await?;
    let node2 = K2Node::new().await?;
    let node3 = K2Node::new().await?;

    // Lấy public key của node1 để làm bootstrap peer
    let node1_id = node1.endpoint.addr().id;
    let node2_id = node2.endpoint.addr().id;

    // Node1 subscribe trước (không có peer)
    let t1 = node1.gossip.subscribe(topic_id, vec![]).await?;
    let (sender1, _rx1) = t1.split();
    node1.cache_sender(topic_id, sender1).await;
    println!("Node #1 subscribed (bootstrap)");

    // Chờ node1 ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Node2 join với node1 làm peer
    let t2 = node2.gossip.subscribe_and_join(topic_id, vec![node1_id]).await?;
    let (_sender2, mut rx2) = t2.split();
    println!("Node #2 joined via Node #1");

    // Node3 join với node1 và node2
    let t3 = node3.gossip.subscribe_and_join(topic_id, vec![node1_id, node2_id]).await?;
    let (_sender3, mut rx3) = t3.split();
    println!("Node #3 joined via Node #1 + #2");

    // Chờ gossip mesh hình thành
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Node1 broadcast
    println!("\n--- Broadcast từ Node #1 ---");
    let payload = b"OFFER_DATA_XYZ_123";
    let start = Instant::now();
    node1.broadcast_message(topic_id, payload.to_vec()).await?;
    println!("Node #1 broadcast xong");

    // Đợi Node2 và Node3 nhận
    let mut received = 0;
    let r2 = tokio::time::timeout(Duration::from_secs(5), rx2.next()).await;
    if matches!(r2, Ok(Some(Ok(_)))) {
        println!("Node #2 nhan duoc sau {}ms", start.elapsed().as_millis());
        received += 1;
    } else {
        println!("Node #2 timeout");
    }

    let r3 = tokio::time::timeout(Duration::from_secs(5), rx3.next()).await;
    if matches!(r3, Ok(Some(Ok(_)))) {
        println!("Node #3 nhan duoc sau {}ms", start.elapsed().as_millis());
        received += 1;
    } else {
        println!("Node #3 timeout");
    }

    println!("\n=== KET QUA ===");
    println!("Ti le nhan: {}/2", received);
    assert!(received > 0, "Gossip khong hoat dong - 0/2 nodes nhan duoc message");

    Ok(())
}

#[test]
fn test_compression_efficiency() -> Result<()> {
    // 1. Tạo một thông điệp Offer có dung lượng lớn (nhiều văn bản)
    let offer = MarketplaceOffer {
        id: K2Marketplace::generate_id(),
        node_id: "node-1234567890-abcdef-ghijk".to_string(),
        topic: "Freelance Jobs".to_string(),
        action: "Offer".to_string(),
        subtopic: Some("Rust Developer".to_string()),
        category: Some("Software".to_string()),
        skill: Some("Rust, P2P, Iroh".to_string()),
        title: "Senior Rust Engineer for P2P Project".to_string(),
        description: "Looking for an expert to build decentralized marketplace using Iroh and Gossip protocols. Must have experience with async Rust.".to_string(),
        price_min: 5000,
        price_max: 10000,
        currency: "USDT".to_string(),
        timestamp: 1675865200,
    };

    let msg = MarketplaceMessage::Offer(offer);

    // 2. Kiểm thử định dạng JSON truyền thống (để so sánh)
    let json_bytes = serde_json::to_vec(&msg)?;
    let json_size = json_bytes.len();

    // 3. Kiểm thử định dạng Postcard (tối ưu hóa của K2)
    let postcard_bytes = K2Marketplace::serialize_message(&msg)?;
    let postcard_size = postcard_bytes.len();

    // 4. Tính toán tỷ lệ nén
    let reduction = 100.0 - (postcard_size as f64 / json_size as f64 * 100.0);

    println!("\n--- Kết quả đo lường hiệu quả nén ---");
    println!("Độ dài tin nhắn mẫu: Dung lượng lớn (Full description)");
    println!("Kích thước JSON:      {} bytes", json_size);
    println!("Kích thước Postcard:  {} bytes", postcard_size);
    println!("Tỷ lệ giảm thiểu:     {:.2}%", reduction);
    println!("--------------------------------------");

    // Đảm bảo Postcard luôn nhỏ hơn JSON và đạt hiệu quả nhất định
    assert!(postcard_size < json_size);
    assert!(reduction > 25.0, "Tỷ lệ nén phải đạt ít nhất 25% so với JSON");

    Ok(())
}

#[tokio::test]
async fn test_broadcast_and_cache_cycle() -> Result<()> {
    // 1. Khởi tạo Node và Topic
    let node = K2Node::new().await?;
    let topic_id = K2Marketplace::topic_to_id("broadcast-test");
    
    // 2. Đăng ký nhận tin và lấy trình xử lý Topic
    let topic = node.subscribe_topic(topic_id).await?;
    
    // 3. Chia tách Topic và lưu trữ Sender vào bộ nhớ đệm (Cache)
    let (sender, _receiver) = topic.split();
    node.cache_sender(topic_id, sender).await;

    // 4. Kiểm thử phát tin bằng cách sử dụng Sender đã lưu trong bộ nhớ đệm
    let test_msg = "Broadcast thông qua cached sender".as_bytes().to_vec();
    let _ = node.broadcast_message(topic_id, test_msg.clone()).await?;

    Ok(())
}
