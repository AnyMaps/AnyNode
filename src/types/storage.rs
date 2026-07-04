use std::collections::VecDeque;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PendingUpload {
    pub country_code: String,
    pub area_id: u32,
    pub file_path: PathBuf,
}

impl PendingUpload {
    pub fn new(country_code: String, area_id: u32, file_path: PathBuf) -> Self {
        Self {
            country_code,
            area_id,
            file_path,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletedUpload {
    pub country_code: String,
    pub area_id: u32,
    pub cid: String,
    pub file_size: u64,
}

impl CompletedUpload {
    pub fn new(country_code: String, area_id: u32, cid: String, file_size: u64) -> Self {
        Self {
            country_code,
            area_id,
            cid,
            file_size,
        }
    }
}

#[derive(Debug)]
pub struct UploadQueue {
    pending_uploads: VecDeque<PendingUpload>,
    batch_size: usize,
    max_queue_size: usize,
}

impl UploadQueue {
    pub fn new(batch_size: usize, max_queue_size: usize) -> Self {
        Self {
            pending_uploads: VecDeque::new(),
            batch_size,
            max_queue_size,
        }
    }

    pub fn add_upload(&mut self, upload: PendingUpload) -> bool {
        if self.pending_uploads.len() >= self.max_queue_size {
            return false;
        }
        self.pending_uploads.push_back(upload);
        true
    }

    pub fn take_batch(&mut self) -> Vec<PendingUpload> {
        let batch_size = std::cmp::min(self.batch_size, self.pending_uploads.len());
        let mut batch = Vec::with_capacity(batch_size);
        for _ in 0..batch_size {
            if let Some(upload) = self.pending_uploads.pop_front() {
                batch.push(upload);
            }
        }
        batch
    }

    pub fn is_batch_ready(&self) -> bool {
        self.pending_uploads.len() >= self.batch_size
    }

    pub fn is_empty(&self) -> bool {
        self.pending_uploads.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pending_uploads.len()
    }
}

#[derive(Debug, Clone, Default)]
pub struct UploadStats {
    pub total_uploaded: u64,
    pub total_failed: u64,
    pub total_bytes_uploaded: u64,
}

impl UploadStats {
    pub fn increment_uploaded(&mut self, bytes: u64) {
        self.total_uploaded += 1;
        self.total_bytes_uploaded += bytes;
    }

    pub fn increment_failed(&mut self) {
        self.total_failed += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pending_upload() -> PendingUpload {
        PendingUpload::new("US".to_string(), 12345, PathBuf::from("/tmp/test.pmtiles"))
    }

    #[test]
    fn pending_upload_new() {
        let upload =
            PendingUpload::new("DE".to_string(), 54321, PathBuf::from("/data/test.pmtiles"));
        assert_eq!(upload.country_code, "DE");
        assert_eq!(upload.area_id, 54321);
        assert_eq!(upload.file_path, PathBuf::from("/data/test.pmtiles"));
    }

    #[test]
    fn completed_upload_new() {
        let completed =
            CompletedUpload::new("FR".to_string(), 11111, "QmTestCid123".to_string(), 98765);
        assert_eq!(completed.country_code, "FR");
        assert_eq!(completed.area_id, 11111);
        assert_eq!(completed.cid, "QmTestCid123");
        assert_eq!(completed.file_size, 98765);
    }

    #[test]
    fn upload_queue_new() {
        let queue = UploadQueue::new(10, 100);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn upload_queue_add_success() {
        let mut queue = UploadQueue::new(10, 100);
        let result = queue.add_upload(sample_pending_upload());
        assert!(result);
    }

    #[test]
    fn upload_queue_add_when_full() {
        let mut queue = UploadQueue::new(10, 3);
        queue.add_upload(sample_pending_upload());
        queue.add_upload(sample_pending_upload());
        queue.add_upload(sample_pending_upload());
        let result = queue.add_upload(sample_pending_upload());
        assert!(!result);
    }

    #[test]
    fn upload_queue_add_increments_len() {
        let mut queue = UploadQueue::new(10, 100);
        assert_eq!(queue.len(), 0);
        queue.add_upload(sample_pending_upload());
        assert_eq!(queue.len(), 1);
        queue.add_upload(sample_pending_upload());
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn upload_queue_take_batch_empty() {
        let mut queue = UploadQueue::new(10, 100);
        let batch = queue.take_batch();
        assert!(batch.is_empty());
    }

    #[test]
    fn upload_queue_take_batch_partial() {
        let mut queue = UploadQueue::new(10, 100);
        queue.add_upload(sample_pending_upload());
        queue.add_upload(sample_pending_upload());
        let batch = queue.take_batch();
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn upload_queue_take_batch_full() {
        let mut queue = UploadQueue::new(3, 100);
        for _ in 0..5 {
            queue.add_upload(sample_pending_upload());
        }
        let batch = queue.take_batch();
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn upload_queue_take_batch_removes_items() {
        let mut queue = UploadQueue::new(3, 100);
        for _ in 0..5 {
            queue.add_upload(sample_pending_upload());
        }
        let batch = queue.take_batch();
        assert_eq!(batch.len(), 3);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn upload_queue_is_batch_ready_false() {
        let mut queue = UploadQueue::new(5, 100);
        queue.add_upload(sample_pending_upload());
        queue.add_upload(sample_pending_upload());
        assert!(!queue.is_batch_ready());
    }

    #[test]
    fn upload_queue_is_batch_ready_true() {
        let mut queue = UploadQueue::new(3, 100);
        queue.add_upload(sample_pending_upload());
        queue.add_upload(sample_pending_upload());
        queue.add_upload(sample_pending_upload());
        assert!(queue.is_batch_ready());
    }

    #[test]
    fn upload_queue_is_empty() {
        let mut queue = UploadQueue::new(10, 100);
        assert!(queue.is_empty());
        queue.add_upload(sample_pending_upload());
        assert!(!queue.is_empty());
        queue.take_batch();
        assert!(queue.is_empty());
    }

    #[test]
    fn upload_queue_len() {
        let mut queue = UploadQueue::new(10, 100);
        assert_eq!(queue.len(), 0);
        queue.add_upload(sample_pending_upload());
        assert_eq!(queue.len(), 1);
        queue.add_upload(sample_pending_upload());
        assert_eq!(queue.len(), 2);
        queue.take_batch();
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn upload_stats_default() {
        let stats = UploadStats::default();
        assert_eq!(stats.total_uploaded, 0);
        assert_eq!(stats.total_failed, 0);
        assert_eq!(stats.total_bytes_uploaded, 0);
    }

    #[test]
    fn upload_stats_record_upload() {
        let mut stats = UploadStats::default();
        stats.increment_uploaded(1000);
        assert_eq!(stats.total_uploaded, 1);
        assert_eq!(stats.total_bytes_uploaded, 1000);
        stats.increment_uploaded(500);
        assert_eq!(stats.total_uploaded, 2);
        assert_eq!(stats.total_bytes_uploaded, 1500);
    }
}
