use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use lofty::{AudioFile, Probe};
use tui::widgets::ListState;

use super::gen_funcs::bulk_add;
use super::playlist::Playlist;
use crate::constants::{SECONDS_PER_DAY, SECONDS_PER_HOUR, SECONDS_PER_MINUTE};

pub struct Queue {
    state: ListState,
    items: VecDeque<PathBuf>,
    curr: usize,
    total_time: u32,
}

impl Queue {
    pub fn with_items() -> Self {
        Self {
            state: ListState::default(),
            items: VecDeque::new(),
            curr: 0,
            total_time: 0,
        }
    }

    // return item at index
    pub fn item(&self) -> Option<&PathBuf> {
        if self.items.is_empty() {
            None
        } else {
            Some(&self.items[self.curr])
        }
    }

    // return all items contained in vector
    pub fn items(&self) -> &VecDeque<PathBuf> {
        &self.items
    }

    pub fn length(&self) -> usize {
        self.items.len()
    }

    pub fn total_time(&self) -> String {
        let days = self.total_time / SECONDS_PER_DAY;
        let hours = (self.total_time % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
        let minutes = (self.total_time % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
        let seconds = self.total_time % SECONDS_PER_MINUTE;

        let mut time_parts = vec![];

        if days > 0 {
            time_parts.push(format!("{days} days"));
        }

        if hours > 0 || days > 0 {
            time_parts.push(format!("{hours} hours"));
        }

        if minutes > 0 || hours > 0 || days > 0 {
            // Include minutes if there are any hours or days
            time_parts.push(format!("{minutes} minutes"));
        }
        if seconds > 0 || time_parts.is_empty() {
            // Always include seconds if there's no other component
            time_parts.push(format!("{seconds} seconds"));
        }

        format!(" Total Length: {} |", time_parts.join(" "))
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn pop(&mut self) -> PathBuf {
        self.decrement_total_time(0);
        let item = self.items.pop_front().unwrap();
        self.curr = self.curr.saturating_sub(1);
        let selected = (!self.items.is_empty()).then_some(self.curr);
        self.state.select(selected);
        item
    }

    pub fn state(&self) -> ListState {
        self.state.clone()
    }

    fn decrement_total_time(&mut self, index: usize) {
        let item = &self.items[index];
        let length = self.item_length(item);
        self.total_time -= length;
    }

    // get audio file length
    pub fn item_length(&self, path: &PathBuf) -> u32 {
        let path = Path::new(&path);
        let tagged_file = Probe::open(path)
            .expect("ERROR: Bad path provided!")
            .read()
            .expect("ERROR: Failed to read file!");

        let properties = &tagged_file.properties();
        let duration = properties.duration();

        // update song length, currently playing
        duration.as_secs() as u32
    }

    pub fn next(&mut self) {
        // check if empty
        if self.items.is_empty() {
            return;
        };

        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.curr = i;
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        // check if empty
        if self.items.is_empty() {
            return;
        };
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.curr = i;
        self.state.select(Some(i));
    }

    pub fn unselect(&mut self) {
        self.state.select(None);
    }

    pub fn add(&mut self, item: PathBuf) {
        if item.is_dir() {
            let files = bulk_add(&item);
            for f in files {
                let length = self.item_length(&f);
                self.total_time += length;
                self.items.push_back(f);
            }
        } else {
            self.total_time += self.item_length(&item);
            self.items.push_back(item);
        }
    }

    // remove item from items vector
    pub fn remove(&mut self) {
        if self.items.is_empty() {
            // top of queue
        } else if self.items.len() == 1 {
            self.decrement_total_time(self.curr);
            self.items.remove(self.curr);
            self.unselect();
        // if at bottom of queue, remove item and select item above above
        } else if self.state.selected().unwrap() >= (self.items.len() - 1) {
            self.decrement_total_time(self.curr);
            self.items.remove(self.curr);
            self.curr -= 1;
            self.state.select(Some(self.curr));
        // else delete item
        } else if !self.items.is_empty() {
            self.decrement_total_time(self.curr);
            self.items.remove(self.curr);
        };
    }

    /// Clear all items from the queue.
    pub fn clear(&mut self) {
        self.items.clear();
        self.total_time = 0;
        self.curr = 0;
        self.unselect();
    }

    /// Create a Playlist from the current queue contents.
    pub fn to_playlist(&self, name: String) -> Playlist {
        Playlist::new(name, self.items.iter().cloned().collect())
    }

    /// Load songs from a playlist into the queue (clears existing queue first).
    /// Returns the number of songs loaded.
    pub fn load_playlist(&mut self, playlist: Playlist) -> usize {
        self.clear();
        let count = playlist.songs.len();
        for song in playlist.songs {
            self.add(song);
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Write a minimal valid PCM WAV file with the given duration of silence.
    fn create_test_wav(path: &Path, duration_secs: u32) {
        let sample_rate: u32 = 44100;
        let bits_per_sample: u16 = 16;
        let num_channels: u16 = 1;
        let bytes_per_sample = (bits_per_sample / 8) as u32;
        let data_size = sample_rate * bytes_per_sample * num_channels as u32 * duration_secs;
        let file_size = 36 + data_size; // RIFF header minus 8 bytes + data

        let mut f = std::fs::File::create(path).unwrap();
        // RIFF header
        f.write_all(b"RIFF").unwrap();
        f.write_all(&file_size.to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        // fmt subchunk
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap(); // subchunk1 size
        f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
        f.write_all(&num_channels.to_le_bytes()).unwrap();
        f.write_all(&sample_rate.to_le_bytes()).unwrap();
        let byte_rate = sample_rate * num_channels as u32 * bytes_per_sample;
        f.write_all(&byte_rate.to_le_bytes()).unwrap();
        let block_align = num_channels * (bits_per_sample / 8);
        f.write_all(&block_align.to_le_bytes()).unwrap();
        f.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        // data subchunk
        f.write_all(b"data").unwrap();
        f.write_all(&data_size.to_le_bytes()).unwrap();
        // silence
        let zeros = vec![0u8; data_size as usize];
        f.write_all(&zeros).unwrap();
    }

    /// Build a Queue by directly populating fields, bypassing `add()`'s
    /// directory-scanning path.
    fn make_queue(paths: &[PathBuf]) -> Queue {
        let mut q = Queue::with_items();
        for p in paths {
            let length = q.item_length(p);
            q.total_time += length;
            q.items.push_back(p.clone());
        }
        if !q.items.is_empty() {
            q.state.select(Some(0));
        }
        q
    }

    enum Op {
        Pop(usize),
        Remove,
    }

    struct Case {
        name: &'static str,
        durations: &'static [u32],
        cursor: usize,
        op: Op,
        expect_total: u32,
        expect_cursor: usize,
        expect_len: usize,
        expect_selected: Option<usize>,
    }

    #[test]
    fn pop_and_remove() {
        let cases = [
            Case {
                name: "pop subtracts front item duration",
                durations: &[3, 5, 7],
                cursor: 2,
                op: Op::Pop(1),
                expect_total: 12,
                expect_cursor: 1,
                expect_len: 2,
                expect_selected: Some(1),
            },
            Case {
                name: "pop adjusts cursor",
                durations: &[1, 1, 1],
                cursor: 2,
                op: Op::Pop(1),
                expect_total: 2,
                expect_cursor: 1,
                expect_len: 2,
                expect_selected: Some(1),
            },
            Case {
                name: "pop with cursor at zero",
                durations: &[2, 4],
                cursor: 0,
                op: Op::Pop(1),
                expect_total: 4,
                expect_cursor: 0,
                expect_len: 1,
                expect_selected: Some(0),
            },
            Case {
                name: "pop last item",
                durations: &[5],
                cursor: 0,
                op: Op::Pop(1),
                expect_total: 0,
                expect_cursor: 0,
                expect_len: 0,
                expect_selected: None,
            },
            Case {
                name: "pop full drain (regression: no panic)",
                durations: &[2, 3, 4],
                cursor: 2,
                op: Op::Pop(3),
                expect_total: 0,
                expect_cursor: 0,
                expect_len: 0,
                expect_selected: None,
            },
            Case {
                name: "remove subtracts cursor item duration",
                durations: &[3, 5, 7],
                cursor: 1,
                op: Op::Remove,
                expect_total: 10,
                expect_cursor: 1,
                expect_len: 2,
                expect_selected: Some(1),
            },
        ];

        for case in &cases {
            let dir = TempDir::new().unwrap();
            let paths: Vec<PathBuf> = case
                .durations
                .iter()
                .enumerate()
                .map(|(i, &d)| {
                    let p = dir.path().join(format!("{i}.wav"));
                    create_test_wav(&p, d);
                    p
                })
                .collect();

            let mut q = make_queue(&paths);
            q.curr = case.cursor;
            q.state.select(Some(case.cursor));

            match case.op {
                Op::Pop(n) => (0..n).for_each(|_| {
                    q.pop();
                }),
                Op::Remove => q.remove(),
            }

            assert_eq!(q.total_time, case.expect_total, "{}: total_time", case.name);
            assert_eq!(q.curr, case.expect_cursor, "{}: curr", case.name);
            assert_eq!(q.length(), case.expect_len, "{}: length", case.name);
            assert_eq!(
                q.state.selected(),
                case.expect_selected,
                "{}: selected",
                case.name
            );
        }
    }
}
