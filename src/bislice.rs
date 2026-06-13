use core::{fmt, iter, mem, ops};

// TODO: make this into it's own crate

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DoubleSlice<'a, T> {
    start: &'a [T],
    end: &'a [T],
}

impl<'a, T> Default for DoubleSlice<'a, T> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            start: Default::default(),
            end: Default::default(),
        }
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for DoubleSlice<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.start)?;
        write!(f, "{:?}", self.end)
    }
}

impl<'a, T> Clone for DoubleSlice<'a, T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            start: self.start,
            end: self.end,
        }
    }
}

impl<'a, T> Copy for DoubleSlice<'a, T> {}

impl<'a, T> DoubleSlice<'a, T> {
    #[inline(always)]
    pub const fn new(start: &'a [T], end: &'a [T]) -> Self {
        Self { start, end }
    }

    #[inline(always)]
    pub const fn from_single(slice: &'a [T]) -> Self {
        Self::new(slice, &[])
    }

    #[inline(always)]
    pub const fn into_slices(self) -> (&'a [T], &'a [T]) {
        (self.start, self.end)
    }
}

impl<'a, T> From<&'a [T]> for DoubleSlice<'a, T> {
    #[inline(always)]
    fn from(value: &'a [T]) -> Self {
        Self::from_single(value)
    }
}

impl<'a, T> From<(&'a [T], &'a [T])> for DoubleSlice<'a, T> {
    #[inline(always)]
    fn from((start, end): (&'a [T], &'a [T])) -> Self {
        Self::new(start, end)
    }
}

impl<'a, T> From<[&'a [T]; 2]> for DoubleSlice<'a, T> {
    #[inline(always)]
    fn from([start, end]: [&'a [T]; 2]) -> Self {
        Self::new(start, end)
    }
}

impl<'a, T> From<DoubleSlice<'a, T>> for (&'a [T], &'a [T]) {
    #[inline(always)]
    fn from(value: DoubleSlice<'a, T>) -> Self {
        value.into_slices()
    }
}

impl<'a, T> From<DoubleSlice<'a, T>> for [&'a [T]; 2] {
    #[inline(always)]
    fn from(value: DoubleSlice<'a, T>) -> Self {
        value.into_slices().into()
    }
}

#[inline(always)]
const fn normalize_range(
    start: ops::Bound<usize>,
    end: ops::Bound<usize>,
    len: usize,
) -> (usize, usize) {
    let start_idx = match start {
        ops::Bound::Included(n) => n,
        ops::Bound::Excluded(n) => n.strict_add(1),
        ops::Bound::Unbounded => 0,
    };

    let end_idx = match end {
        ops::Bound::Included(n) => n.strict_add(1),
        ops::Bound::Excluded(n) => n,
        ops::Bound::Unbounded => len,
    };

    (start_idx, end_idx)
}

impl<'a, T> DoubleSlice<'a, T> {
    #[inline(always)]
    pub const fn len(self) -> usize {
        let (start, end) = self.into_slices();
        start.len().strict_add(end.len())
    }

    #[inline(always)]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn get(self, index: usize) -> Option<&'a T> {
        let (start, end) = self.into_slices();
        if let Some(i) = index.checked_sub(start.len()) {
            end.get(i)
        } else {
            start.get(index)
        }
    }

    #[inline(always)]
    pub fn slice(self, range: impl ops::RangeBounds<usize>) -> Option<Self> {
        let this_len = self.len();
        let (mut start, mut end) = self.into_slices();

        let (start_idx, end_idx) = normalize_range(
            range.start_bound().cloned(),
            range.end_bound().cloned(),
            this_len,
        );

        let range_len = end_idx.checked_sub(start_idx)?;

        if let Some(k) = start_idx.checked_sub(start.len()) {
            start = &[];
            end = end.get(k..)?;
        } else {
            start = &start[start_idx..];
        }

        if let Some(k) = range_len.checked_sub(start.len()) {
            end = end.get(..k)?;
        } else {
            end = &[];
            start = &start[..range_len];
        }

        Some(Self::new(start, end))
    }

    #[inline(always)]
    pub fn split_at(self, mid: usize) -> Option<(Self, Self)> {
        let split = self.start.len();

        self.start
            .split_at_checked(mid)
            .map(|(start, end)| (Self::from_single(start), Self::new(end, self.end)))
            .or_else(|| {
                self.end
                    .split_at_checked(mid.strict_sub(split))
                    .map(|(start, end)| (Self::new(self.start, start), Self::from_single(end)))
            })
    }

    #[inline(always)]
    pub fn split_first(self) -> Option<(&'a T, Self)> {
        self.start
            .split_first()
            .map(|(first, rem)| (first, Self::new(rem, self.end)))
            .or_else(|| {
                self.end
                    .split_first()
                    .map(|(first, rem)| (first, Self::from_single(rem)))
            })
    }

    #[inline(always)]
    pub fn split_last(self) -> Option<(&'a T, Self)> {
        self.end
            .split_last()
            .map(|(last, rem)| (last, Self::new(self.start, rem)))
            .or_else(|| {
                self.start
                    .split_last()
                    .map(|(first, rem)| (first, Self::from_single(rem)))
            })
    }

    #[inline(always)]
    pub fn iter(self) -> Iter<'a, T> {
        Iter::from_slice(self)
    }
}

impl<'a, T> ops::Index<usize> for DoubleSlice<'a, T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("index out of bounds")
    }
}

pub struct Iter<'a, T> {
    start: core::slice::Iter<'a, T>,
    end: core::slice::Iter<'a, T>,
}

impl<'a, T: fmt::Debug> fmt::Debug for Iter<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Iter")
            .field(&self.start.as_slice())
            .field(&self.end.as_slice())
            .finish()
    }
}

impl<'a, T> Clone for Iter<'a, T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            start: self.start.clone(),
            end: self.end.clone(),
        }
    }
}

impl<'a, T> Default for Iter<'a, T> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            start: Default::default(),
            end: Default::default(),
        }
    }
}

// TODO: make these const when the relevant std functions become const
impl<'a, T> Iter<'a, T> {
    #[inline(always)]
    fn new(first: &'a [T], second: &'a [T]) -> Self {
        Self {
            start: first.iter(),
            end: second.iter(),
        }
    }

    #[inline(always)]
    fn from_slice(slice: DoubleSlice<'a, T>) -> Self {
        let (first, second) = slice.into_slices();
        Self::new(first, second)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> DoubleSlice<'_, T> {
        DoubleSlice::new(self.start.as_slice(), self.end.as_slice())
    }
}

impl<'a, T> IntoIterator for DoubleSlice<'a, T> {
    type IntoIter = Iter<'a, T>;
    type Item = <Self::IntoIter as Iterator>::Item;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(v) = self.start.next() {
            return Some(v);
        }
        self.end.next()
    }

    #[inline(always)]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let split = self.start.as_slice().len();

        if let Some(x) = self.start.nth(n) {
            return Some(x);
        }

        self.end.nth(n.strict_sub(split))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(v) = self.end.next_back() {
            return Some(v);
        }
        self.start.next_back()
    }

    #[inline(always)]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let rsplit = self.end.as_slice().len();

        if let Some(x) = self.end.nth_back(n) {
            return Some(x);
        }

        self.start.nth_back(n.strict_sub(rsplit))
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.as_ref().len()
    }
}

impl<'a, T> iter::FusedIterator for Iter<'a, T> {}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DoubleSliceMut<'a, T> {
    start: &'a mut [T],
    end: &'a mut [T],
}

impl<'a, T> Default for DoubleSliceMut<'a, T> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            start: Default::default(),
            end: Default::default(),
        }
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for DoubleSliceMut<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.start)?;
        write!(f, "{:?}", self.end)
    }
}

impl<'a, T> DoubleSliceMut<'a, T> {
    #[inline(always)]
    pub const fn new(start: &'a mut [T], end: &'a mut [T]) -> Self {
        Self { start, end }
    }

    #[inline(always)]
    pub const fn from_single(slice: &'a mut [T]) -> Self {
        Self::new(slice, &mut [])
    }

    #[inline(always)]
    pub const fn into_mut_slices(self) -> (&'a mut [T], &'a mut [T]) {
        (self.start, self.end)
    }
}

impl<'a, T> From<&'a mut [T]> for DoubleSliceMut<'a, T> {
    #[inline(always)]
    fn from(value: &'a mut [T]) -> Self {
        Self::from_single(value)
    }
}

impl<'a, T> From<(&'a mut [T], &'a mut [T])> for DoubleSliceMut<'a, T> {
    #[inline(always)]
    fn from((start, end): (&'a mut [T], &'a mut [T])) -> Self {
        Self::new(start, end)
    }
}

impl<'a, T> From<[&'a mut [T]; 2]> for DoubleSliceMut<'a, T> {
    #[inline(always)]
    fn from([start, end]: [&'a mut [T]; 2]) -> Self {
        Self::new(start, end)
    }
}

impl<'a, T> From<DoubleSliceMut<'a, T>> for (&'a mut [T], &'a mut [T]) {
    #[inline(always)]
    fn from(value: DoubleSliceMut<'a, T>) -> Self {
        value.into_mut_slices()
    }
}

impl<'a, T> From<DoubleSliceMut<'a, T>> for [&'a mut [T]; 2] {
    #[inline(always)]
    fn from(value: DoubleSliceMut<'a, T>) -> Self {
        value.into_mut_slices().into()
    }
}

impl<'a, T> DoubleSliceMut<'a, T> {
    #[inline(always)]
    pub const fn reborrow_mut(&mut self) -> DoubleSliceMut<'_, T> {
        DoubleSliceMut {
            start: &mut self.start,
            end: &mut self.end,
        }
    }

    #[inline(always)]
    pub const fn reborrow(&self) -> DoubleSlice<'_, T> {
        DoubleSlice {
            start: &self.start,
            end: &self.end,
        }
    }

    #[inline(always)]
    pub const fn into_ref(self) -> DoubleSlice<'a, T> {
        let (start, end) = self.into_mut_slices();
        DoubleSlice { start, end }
    }

    #[inline(always)]
    pub fn get_mut(self, index: usize) -> Option<&'a mut T> {
        let (start, end) = self.into_mut_slices();
        if let Some(i) = index.checked_sub(start.len()) {
            end.get_mut(i)
        } else {
            start.get_mut(index)
        }
    }

    #[inline(always)]
    pub fn slice_mut(self, range: impl ops::RangeBounds<usize>) -> Option<Self> {
        let this_len = self.reborrow().len();
        let (mut start, mut end) = self.into_mut_slices();

        let (start_idx, end_idx) = normalize_range(
            range.start_bound().cloned(),
            range.end_bound().cloned(),
            this_len,
        );

        let range_len = end_idx.checked_sub(start_idx)?;

        if let Some(k) = start_idx.checked_sub(start.len()) {
            start = &mut [];
            end = end.get_mut(k..)?;
        } else {
            start = &mut start[start_idx..];
        }

        if let Some(k) = range_len.checked_sub(start.len()) {
            end = end.get_mut(..k)?;
        } else {
            end = &mut [];
            start = &mut start[..range_len];
        }

        Some(Self::new(start, end))
    }

    #[inline(always)]
    #[must_use]
    pub fn copy_from_slice(self, source: DoubleSlice<'a, T>) -> Option<()>
    where
        T: Copy,
    {
        if self.reborrow().len() != source.len() {
            return None;
        }

        let (src_start, src_end) = source.into_slices();
        let (dest_start, dest_end) = self.into_mut_slices();

        let src_split = src_start.len();
        let dest_split = dest_start.len();

        let mid_len = dest_split.abs_diff(src_split);

        if src_split >= dest_split {
            let (src_start, src_mid) = src_start.split_at_checked(dest_split).unwrap();
            let (dest_mid, dest_end) = dest_end.split_at_mut_checked(mid_len).unwrap();
            dest_start.copy_from_slice(src_start);
            dest_mid.copy_from_slice(src_mid);
            dest_end.copy_from_slice(src_end);
        } else {
            let (src_mid, src_end) = src_end.split_at_checked(mid_len).unwrap();
            let (dest_start, dest_mid) = dest_start.split_at_mut_checked(src_split).unwrap();
            dest_start.copy_from_slice(src_start);
            dest_mid.copy_from_slice(src_mid);
            dest_end.copy_from_slice(src_end);
        }

        Some(())
    }

    #[inline(always)]
    pub fn split_at(self, mid: usize) -> Option<(Self, Self)> {
        if let Some(i) = mid.checked_sub(self.start.len()) {
            self.end
                .split_at_mut_checked(i)
                .map(|(start, end)| (Self::new(self.start, start), Self::from_single(end)))
        } else {
            let (start, end) = self.start.split_at_mut(mid);
            Some((Self::from_single(start), Self::new(end, self.end)))
        }
    }

    #[inline(always)]
    pub fn split_first(self) -> Option<(&'a mut T, Self)> {
        if let Some((first, rem)) = self.start.split_first_mut() {
            Some((first, Self::new(rem, self.end)))
        } else {
            self.end
                .split_first_mut()
                .map(|(first, rem)| (first, Self::from_single(rem)))
        }
    }

    #[inline(always)]
    pub fn split_last(self) -> Option<(&'a mut T, Self)> {
        if let Some((last, rem)) = self.end.split_last_mut() {
            Some((last, Self::new(self.start, rem)))
        } else {
            self.start
                .split_last_mut()
                .map(|(first, rem)| (first, Self::from_single(rem)))
        }
    }

    #[inline(always)]
    pub fn iter_mut(self) -> IterMut<'a, T> {
        IterMut::from_slice(self)
    }
}

impl<'a, T> DoubleSliceMut<'a, mem::MaybeUninit<T>> {
    #[inline(always)]
    #[must_use]
    pub fn write_copy_of_slice(self, slice: DoubleSlice<'_, T>) -> Option<DoubleSliceMut<'a, T>>
    where
        T: Copy,
    {
        if self.reborrow().len() != slice.len() {
            return None;
        }

        let (src_start, src_end) = slice.into_slices();
        let (dest_start, dest_end) = self.into_mut_slices();

        let src_split = src_start.len();
        let dest_split = dest_start.len();

        let mid_len = dest_split.abs_diff(src_split);

        if src_split >= dest_split {
            let (src_start, src_mid) = src_start.split_at_checked(dest_split).unwrap();
            let (dest_mid, dest_end) = dest_end.split_at_mut_checked(mid_len).unwrap();
            dest_start.write_copy_of_slice(src_start);
            dest_mid.write_copy_of_slice(src_mid);
            dest_end.write_copy_of_slice(src_end);
        } else {
            let (src_mid, src_end) = src_end.split_at_checked(mid_len).unwrap();
            let (dest_start, dest_mid) = dest_start.split_at_mut_checked(src_split).unwrap();
            dest_start.write_copy_of_slice(src_start);
            dest_mid.write_copy_of_slice(src_mid);
            dest_end.write_copy_of_slice(src_end);
        }

        Some(DoubleSliceMut::new(
            unsafe { dest_start.assume_init_mut() },
            unsafe { dest_end.assume_init_mut() },
        ))
    }
}

impl<'a, T> ops::Index<usize> for DoubleSliceMut<'a, T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        self.reborrow().get(index).expect("index out of bounds")
    }
}

impl<'a, T> ops::IndexMut<usize> for DoubleSliceMut<'a, T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.reborrow_mut()
            .get_mut(index)
            .expect("index out of bounds")
    }
}

pub struct IterMut<'a, T> {
    start: core::slice::IterMut<'a, T>,
    end: core::slice::IterMut<'a, T>,
}

impl<'a, T: fmt::Debug> fmt::Debug for IterMut<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IterMut")
            .field(&self.start.as_slice())
            .field(&self.end.as_slice())
            .finish()
    }
}

impl<'a, T> Default for IterMut<'a, T> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            start: Default::default(),
            end: Default::default(),
        }
    }
}

// TODO: make these const when the relevant std functions become const
impl<'a, T> IterMut<'a, T> {
    #[inline(always)]
    fn new(first: &'a mut [T], second: &'a mut [T]) -> Self {
        Self {
            start: first.iter_mut(),
            end: second.iter_mut(),
        }
    }

    #[inline(always)]
    fn from_slice(slice: DoubleSliceMut<'a, T>) -> Self {
        let (first, second) = slice.into_mut_slices();
        Self::new(first, second)
    }

    #[inline(always)]
    pub fn into_slice(self) -> DoubleSliceMut<'a, T> {
        DoubleSliceMut::new(self.start.into_slice(), self.end.into_slice())
    }

    #[inline(always)]
    pub fn as_ref(&self) -> DoubleSlice<'_, T> {
        DoubleSlice::new(self.start.as_slice(), self.end.as_slice())
    }

    // TODO: as_mut when the as_mut_slice std function on single slice iterators lands
}

impl<'a, T> IntoIterator for DoubleSliceMut<'a, T> {
    type IntoIter = IterMut<'a, T>;
    type Item = <Self::IntoIter as Iterator>::Item;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(v) = self.start.next() {
            return Some(v);
        }
        self.end.next()
    }

    #[inline(always)]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let split = self.start.as_slice().len();

        if let Some(x) = self.start.nth(n) {
            return Some(x);
        }

        self.end.nth(n.strict_sub(split))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(v) = self.end.next_back() {
            return Some(v);
        }
        self.start.next_back()
    }

    #[inline(always)]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let rsplit = self.end.as_slice().len();

        if let Some(x) = self.end.nth_back(n) {
            return Some(x);
        }

        self.start.nth_back(n.strict_sub(rsplit))
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.as_ref().len()
    }
}

impl<'a, T> iter::FusedIterator for IterMut<'a, T> {}
