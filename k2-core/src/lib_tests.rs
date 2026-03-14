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
    
    let mut nodes = Vec::new();
    let mut receivers = Vec::new();

    // 1. GIA NHẬP MẠNG (Discovery Phase)
    for i in 1..=3 {
        let node = K2Node::new().await?;
        let start = Instant::now();
        
        // Subscribe và lấy receiver để đo tin nhắn
        let gossip_topic = node.subscribe_topic_with_discovery(topic_id).await?;
        let (_, rx) = gossip_topic.split();
        
        println!("Node #{} Join: {:?}ms", i, start.elapsed().as_millis());
        nodes.push(node);
        receivers.push(rx);
        
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    // 2. LAN TRUYỀN TIN NHẮN (Gossip Phase)
    println!("\n--- Đo lường độ trễ lan truyền tin nhắn ---");
    let ping_payload = b"OFFER_DATA_XYZ_123";
    let start_broadcast = Instant::now();
    
    // Node 1 phát tán một "Offer"
    nodes[0].broadcast_message(topic_id, ping_payload.to_vec()).await?;
    println!("Node #1 đã phát tin nhắn (Broadcast)");

    // Đợi Node 2 và 3 nhận được
    let mut received_count = 0;
    for i in 1..3 {
        let mut rx = receivers.remove(1); 
        let node_idx = i + 1;
        
        match tokio::time::timeout(Duration::from_secs(5), rx.next()).await {
            Ok(Some(Ok(_event))) => {
                let latency = start_broadcast.elapsed().as_millis();
                println!("Node #{} nhận được tin nhắn sau: {}ms", node_idx, latency);
                received_count += 1;
            },
            _ => println!("Node #{} không nhận được tin nhắn (Timeout)", node_idx),
        }
    }

    println!("\n=== TỔNG KẾT HIỆU NĂNG ===");
    println!("Tỷ lệ nhận tin: {}/2", received_count);
    println!("Độ trễ trung bình: ... ms"); // Bạn có thể tính toán từ kết quả trên
    println!("===========================================\n");

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
