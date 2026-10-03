/*
 * Somebody's own settings: what they see, how the home page opens and how
 * films reach them, and what Melyxar is. Nothing here changes the server for anybody else, which
 * is the whole of what separates it from the administration.
 */

import { Sectioned } from "../../components/sectioned";
import type { SectionGroup } from "../../components/sectioned";
import {
  BellIcon,
  HomeIcon,
  InfoIcon,
  MusicIcon,
  PaletteIcon,
  PlaybackIcon,
  ProfileIcon,
  SubtitlesIcon,
} from "../../icons";

const SECTIONS: SectionGroup[] = [
  {
    sections: [
      { path: "", icon: ProfileIcon, label: "me.profile" },
      { path: "appearance", icon: PaletteIcon, label: "me.appearance" },
      { path: "home", icon: HomeIcon, label: "me.home" },
      { path: "playback", icon: PlaybackIcon, label: "me.playback" },
      { path: "subtitles", icon: SubtitlesIcon, label: "me.subtitles" },
      { path: "music", icon: MusicIcon, label: "me.music" },
      { path: "notifications", icon: BellIcon, label: "me.notifications" },
      { path: "about", icon: InfoIcon, label: "me.about" },
    ],
  },
];

export function MySettingsLayout() {
  return <Sectioned base="/settings" place="me.title" groups={SECTIONS} />;
}
