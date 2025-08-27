use crate::status::RepositoryStatus;

pub struct NavigationState {
    current_section: StatusSection,
    selected_index: usize,
    sections: Vec<SectionInfo>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StatusSection {
    Staged,
    Unstaged,
    Untracked,
    Conflicted,
}

#[derive(Clone, Debug)]
struct SectionInfo {
    section_type: StatusSection,
    file_count: usize,
    start_index: usize,
}

impl NavigationState {
    pub fn new(status: &RepositoryStatus) -> Self {
        let sections = Self::build_sections(status);
        let current_section = sections
            .first()
            .map(|s| s.section_type)
            .unwrap_or(StatusSection::Staged);

        Self {
            current_section,
            selected_index: 0,
            sections,
        }
    }

    pub fn update_status(&mut self, status: &RepositoryStatus) {
        let new_sections = Self::build_sections(status);
        let current_global_index = self.get_global_index();

        self.sections = new_sections;

        if let Some(new_position) = self.find_closest_position(current_global_index) {
            self.current_section = new_position.0;
            self.selected_index = new_position.1;
        } else {
            self.reset_to_first_available();
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else if let Some(prev_section) = self.find_previous_non_empty_section() {
            self.current_section = prev_section;
            self.selected_index = self
                .get_current_section_info()
                .map(|s| s.file_count.saturating_sub(1))
                .unwrap_or(0);
        }
    }

    pub fn move_down(&mut self) {
        if let Some(section_info) = self.get_current_section_info() {
            if self.selected_index + 1 < section_info.file_count {
                self.selected_index += 1;
            } else if let Some(next_section) = self.find_next_non_empty_section() {
                self.current_section = next_section;
                self.selected_index = 0;
            }
        }
    }

    pub fn move_to_top(&mut self) {
        self.reset_to_first_available();
    }

    pub fn move_to_bottom(&mut self) {
        if let Some(last_section) = self.sections.last()
            && last_section.file_count > 0
        {
            self.current_section = last_section.section_type;
            self.selected_index = last_section.file_count - 1;
        }
    }

    pub fn current_section(&self) -> StatusSection {
        self.current_section
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn get_global_index(&self) -> usize {
        let current_section_start = self
            .sections
            .iter()
            .find(|s| s.section_type == self.current_section)
            .map(|s| s.start_index)
            .unwrap_or(0);

        current_section_start + self.selected_index
    }

    pub fn has_selections(&self) -> bool {
        !self.sections.is_empty() && self.sections.iter().any(|s| s.file_count > 0)
    }

    fn build_sections(status: &RepositoryStatus) -> Vec<SectionInfo> {
        let mut sections = Vec::new();
        let mut start_index = 0;

        let section_configs = [
            (StatusSection::Staged, status.staged_files().len()),
            (StatusSection::Unstaged, status.unstaged_files().len()),
            (StatusSection::Untracked, status.untracked_files().len()),
            (StatusSection::Conflicted, status.conflicted_files().len()),
        ];

        for (section_type, count) in section_configs {
            if count > 0 {
                sections.push(SectionInfo {
                    section_type,
                    file_count: count,
                    start_index,
                });
                start_index += count;
            }
        }

        sections
    }

    fn get_current_section_info(&self) -> Option<&SectionInfo> {
        self.sections
            .iter()
            .find(|s| s.section_type == self.current_section)
    }

    fn find_previous_non_empty_section(&self) -> Option<StatusSection> {
        let section_order = [
            StatusSection::Staged,
            StatusSection::Unstaged,
            StatusSection::Untracked,
            StatusSection::Conflicted,
        ];

        let current_pos = section_order
            .iter()
            .position(|&s| s == self.current_section)?;

        for i in (0..current_pos).rev() {
            let section = section_order[i];
            if self
                .sections
                .iter()
                .any(|s| s.section_type == section && s.file_count > 0)
            {
                return Some(section);
            }
        }
        None
    }

    fn find_next_non_empty_section(&self) -> Option<StatusSection> {
        let section_order = [
            StatusSection::Staged,
            StatusSection::Unstaged,
            StatusSection::Untracked,
            StatusSection::Conflicted,
        ];

        let current_pos = section_order
            .iter()
            .position(|&s| s == self.current_section)?;

        for section in section_order.iter().skip(current_pos + 1) {
            let section = *section;
            if self
                .sections
                .iter()
                .any(|s| s.section_type == section && s.file_count > 0)
            {
                return Some(section);
            }
        }
        None
    }

    fn find_closest_position(&self, target_global_index: usize) -> Option<(StatusSection, usize)> {
        for section in &self.sections {
            let section_end = section.start_index + section.file_count;
            if target_global_index >= section.start_index && target_global_index < section_end {
                return Some((
                    section.section_type,
                    target_global_index - section.start_index,
                ));
            }
        }

        if let Some(last_section) = self.sections.last()
            && last_section.file_count > 0
        {
            return Some((last_section.section_type, last_section.file_count - 1));
        }

        None
    }

    fn reset_to_first_available(&mut self) {
        if let Some(first_section) = self.sections.first()
            && first_section.file_count > 0
        {
            self.current_section = first_section.section_type;
            self.selected_index = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_empty_status() {
        let status = RepositoryStatus::empty();
        let nav = NavigationState::new(&status);

        assert!(!nav.has_selections());
    }

    #[test]
    fn test_navigation_single_section() {
        let status = RepositoryStatus::empty();
        let nav = NavigationState::new(&status);

        assert_eq!(nav.current_section(), StatusSection::Staged);
        assert_eq!(nav.selected_index(), 0);
    }

    #[test]
    fn test_move_up_down() {
        let status = RepositoryStatus::empty();
        let mut nav = NavigationState::new(&status);

        nav.move_down();
        nav.move_up();

        assert_eq!(nav.selected_index(), 0);
    }
}
