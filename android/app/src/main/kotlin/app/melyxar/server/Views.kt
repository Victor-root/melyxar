package app.melyxar.server

import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonElement

/*
 * What the server answers, as its own views write it (crates/server), with
 * only what the application reads of them. Names are read in snake case, see
 * `Api.kt`.
 */

/** One of the sizes a picture was made in. */
@Serializable
data class Picture(val url: String, val width: Int? = null, val height: Int? = null)

/** The smallest size of a picture with a pixel for every pixel it covers,
 *  or the largest there is: the server only makes sizes no larger than what
 *  it was given. A size of unknown width is taken for the largest. */
fun List<Picture>.fitting(pixelsAcross: Int): Picture? {
    val bySize = sortedBy { it.width ?: Int.MAX_VALUE }
    return bySize.firstOrNull { (it.width ?: Int.MAX_VALUE) >= pixelsAcross } ?: bySize.lastOrNull()
}

/** An account offered at the door. */
@Serializable
data class DoorName(val name: String, val avatar: String? = null)

@Serializable
data class DoorNames(val names: List<DoorName> = emptyList())

@Serializable
data class Account(
    val id: String,
    val name: String,
    val isAdministrator: Boolean = false,
    val avatar: String? = null,
)

/** The account's own choices that change how the application looks and
 *  plays. */
@Serializable
data class Preferences(
    val interfaceLanguage: String = "auto",
    /** "dark", "light", "system" or "server". */
    val themeMode: String = "server",
    val accentColor: String = "#c81e1e",
    val homeSections: List<String> = emptyList(),
    val hiddenHomeSections: List<String> = emptyList(),
    val stepBackSeconds: Int = 10,
    val stepOnSeconds: Int = 30,
    /** What is drawn behind the pages: "light", "library" or "none". */
    val backdrop: String = "light",
    /** Which painting of light, from one. */
    val backdropLight: Int = 1,
    /** Every button of the bar and of the account's menu, in the order both
     *  show them, and those shown on the bar. */
    val headerButtons: List<String> = emptyList(),
    val buttonsInTheBar: List<String> = listOf("search", "favourites", "watch_later", "notifications"),
    /** Three letter codes, nothing for no preference. */
    val preferredAudioLanguage: String? = null,
    val preferredSubtitleLanguage: String? = null,
    /** When a film starts with subtitles nobody picked for it. */
    val subtitleMode: String = "smart",
    val subtitleModes: List<String> = emptyList(),
    /** How far back a film starts from where it was left. */
    val resumeRewindSeconds: Int = 0,
    /** The longest a step, or the way back on resuming, may be. */
    val longestStep: Int = 90,
    /** What is done with a film of wide gamut colour, and every choice. */
    val wideGamut: String = "automatic",
    val wideGamutChoices: List<String> = emptyList(),
    /** The languages the libraries hold, which are what is offered. */
    val audioLanguages: List<String> = emptyList(),
    val subtitleLanguages: List<String> = emptyList(),
)

/** A work as every row and grid shows it. */
@Serializable
data class Card(
    val id: String,
    val title: String,
    val year: Int? = null,
    val runtimeMinutes: Int? = null,
    val rating: Double? = null,
    /** The colour shown while the picture comes, and in its place. */
    val color: String? = null,
    val poster: List<Picture> = emptyList(),
    /** Lying down, for the rows that show works so. */
    val wide: List<Picture> = emptyList(),
    /** movie, series, season, episode, folder, video or photo. */
    val kind: String,
    val initial: String = "",
    val library: String = "",
    /** not_started, in_progress or watched. */
    val seen: String = "not_started",
    val watchedMarks: Boolean = true,
    val resumeFromSeconds: Long? = null,
    val resumeLengthSeconds: Long? = null,
    val favourite: Boolean = false,
    val watchLater: Boolean = false,
    val episodes: Int = 0,
    val unwatched: Int = 0,
    /** The file played straight from a card, when it has one. */
    val source: String? = null,
)

/**
 * A card and what a row adds to it, both read from the same object: the
 * server writes the card's fields beside the row's own.
 */
@Serializable(with = WithCardSerializer::class)
data class WithCard<More>(val card: Card, val more: More)

class WithCardSerializer<More>(private val moreSerializer: KSerializer<More>) : KSerializer<WithCard<More>> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("app.melyxar.server.WithCard")

    override fun deserialize(decoder: Decoder): WithCard<More> {
        val json = decoder as JsonDecoder
        val element = json.decodeJsonElement()
        return WithCard(
            json.json.decodeFromJsonElement(Card.serializer(), element),
            json.json.decodeFromJsonElement(moreSerializer, element),
        )
    }

    override fun serialize(encoder: Encoder, value: WithCard<More>) =
        throw UnsupportedOperationException("only ever read")
}

/** What the banner of the home screen says of a work. */
@Serializable
data class HeroMore(
    val backdrop: List<Picture> = emptyList(),
    val logo: List<Picture> = emptyList(),
    val tagline: String? = null,
    val overview: String? = null,
    val genres: List<String> = emptyList(),
    val width: Int? = null,
    val height: Int? = null,
    /** hdr10, hlg or dolby_vision. */
    val hdr: String? = null,
    val sound: String? = null,
    val seriesTitle: String? = null,
    val seasonNumber: Int? = null,
    val episodeNumber: Int? = null,
)

/** Where an episode stands in its series. */
@Serializable
data class EpisodeMore(
    val seriesTitle: String? = null,
    val seasonNumber: Int? = null,
    val episodeNumber: Int? = null,
)

/** The works of one kind, for the category strip under the banner. */
@Serializable
data class Shelf(val kind: String, val cards: List<Card> = emptyList(), val fan: List<Card> = emptyList())

@Serializable
data class Home(
    val hero: List<WithCard<HeroMore>> = emptyList(),
    val carryOn: List<WithCard<EpisodeMore>> = emptyList(),
    val upNext: List<WithCard<EpisodeMore>> = emptyList(),
    val recentlyAdded: List<Card> = emptyList(),
    val shelves: List<Shelf> = emptyList(),
    /** band, carry_on, up_next, recently_added and newest:<kind>, in the
     *  account's order, those it hid left out. */
    val sections: List<String> = emptyList(),
    val kindOrder: List<String> = emptyList(),
)

@Serializable
data class Library(val id: String, val name: String, val kind: String, val works: Int = 0, val version: Long = 0)

/** A page of a grid, and where the next one starts. */
@Serializable
data class CardPage(val cards: List<Card> = emptyList(), val next: String? = null)

@Serializable
data class Credit(
    val personId: String,
    val name: String,
    val role: String,
    val character: String? = null,
    val photo: List<Picture> = emptyList(),
)

@Serializable
data class VideoTrack(
    val codec: String,
    val profile: String? = null,
    val width: Int = 0,
    val height: Int = 0,
    val hdr: String? = null,
    /** The title the file itself gives the track, when it gives one. */
    val title: String? = null,
    val level: Int? = null,
    val aspectRatio: String? = null,
    val isInterlaced: Boolean = false,
    val frameRate: Double? = null,
    val bitrate: Long? = null,
    val pixelFormat: String? = null,
    val referenceFrames: Int? = null,
    val colorPrimaries: String? = null,
    val colorSpace: String? = null,
    val colorTransfer: String? = null,
    val bitDepth: Int? = null,
    val isDefault: Boolean = false,
)

@Serializable
data class AudioTrack(
    val codec: String,
    val profile: String? = null,
    val language: String? = null,
    val channels: Int = 0,
    val isDefault: Boolean = false,
    val title: String? = null,
    val channelLayout: String? = null,
    val sampleRate: Int? = null,
    val bitDepth: Int? = null,
    val bitrate: Long? = null,
    val isForced: Boolean = false,
)

@Serializable
data class SubtitleTrack(
    val codec: String,
    val title: String? = null,
    val language: String? = null,
    val isDefault: Boolean = false,
    val isForced: Boolean = false,
    val isHearingImpaired: Boolean = false,
    val isExternal: Boolean = false,
    /** Painted into the picture by the server to be shown at all. */
    val burnsIn: Boolean = false,
)

@Serializable
data class Version(
    val id: String,
    val summary: String = "",
    val durationMinutes: Int? = null,
    val missing: Boolean = false,
    val video: List<VideoTrack> = emptyList(),
    val audio: List<AudioTrack> = emptyList(),
    val subtitles: List<SubtitleTrack> = emptyList(),
    /** Where the file is and the disk it is on, sent to an administrator
     *  only. */
    val path: String? = null,
    val rootLabel: String? = null,
    val addedAt: String? = null,
    val sizeBytes: Long = 0,
    val overallBitrate: Long? = null,
    val container: String? = null,
    val analysed: Boolean = true,
    val chapters: Int = 0,
)

@Serializable
data class Rating(val source: String, val value: Double)

/** A season of a series, or an episode of a season. */
@Serializable
data class Child(
    val number: Int? = null,
    val title: String? = null,
    val overview: String? = null,
    val childCount: Int = 0,
    val card: Card,
)

@Serializable
data class NextEpisode(
    val id: String,
    val season: Int? = null,
    val episode: Int? = null,
    val title: String? = null,
    val sourceId: String? = null,
)

@Serializable
data class Ancestor(
    val id: String,
    val kind: String,
    val number: Int? = null,
    val title: String? = null,
    /** A series' title drawn as a picture, which its episodes have none of. */
    val logo: List<Picture> = emptyList(),
)

/** Works shown beside one, under a name. */
@Serializable
data class Kin(val name: String? = null, val genre: String? = null, val cards: List<Card> = emptyList())

@Serializable
data class Work(
    val id: String,
    val kind: String,
    val title: String,
    val tagline: String? = null,
    val overview: String? = null,
    val year: Int? = null,
    val releaseDate: String? = null,
    val endDate: String? = null,
    val runtimeMinutes: Int? = null,
    /** The catalogue's own rating, out of ten. */
    val rating: Double? = null,
    val ageRating: String? = null,
    val color: String? = null,
    val number: Int? = null,
    val genres: List<String> = emptyList(),
    val cast: List<Credit> = emptyList(),
    val poster: List<Picture> = emptyList(),
    val backdrop: List<Picture> = emptyList(),
    val logo: List<Picture> = emptyList(),
    val versions: List<Version> = emptyList(),
    val ratings: List<Rating> = emptyList(),
    val children: List<Child> = emptyList(),
    /** The series and season above an episode, the series first. */
    val ancestry: List<Ancestor> = emptyList(),
    /** Whether [title] says more than the number of a season or an episode;
     *  a scan that read only a number wrote it out in English. */
    val hasOwnName: Boolean = true,
    /** An episode's season, itself among them. */
    val siblings: List<Child> = emptyList(),
    val carryOnWith: NextEpisode? = null,
    /** The photos before and after a photo in its folder, when there are. */
    val previousPhoto: String? = null,
    val nextPhoto: String? = null,
    val card: Card? = null,
    val alike: Kin? = null,
    val saga: Kin? = null,
    val trailers: List<WorkTrailer> = emptyList(),
)

/** A trailer of a work: a file next to it, fetched from [url], or a link
 *  watched where it lives, [remoteUrl]. */
@Serializable
data class WorkTrailer(val url: String? = null, val remoteUrl: String? = null)

/** A track of a film as the player offers it. */
@Serializable
data class Track(
    val id: String,
    val language: String? = null,
    val title: String? = null,
    val codec: String = "",
    val isDefault: Boolean = false,
    val channels: Int? = null,
    /** A subtitle drawn as pictures, which has to be painted into the film. */
    val burnsIn: Boolean = false,
    /** Where a subtitle of words is fetched on its own, as WebVTT. */
    val url: String? = null,
)

/** A stretch of a film a viewer may want to skip. */
@Serializable
data class Segment(val kind: String, val fromSecond: Double, val toSecond: Double)

/** How the server will hand a film over. */
@Serializable
data class Plan(
    /** The file itself, for a film played as it is. */
    val url: String,
    /** direct_play, remux, transcode_audio or full_transcode. */
    val method: String,
    val chosenAudioId: String? = null,
    val chosenSubtitleId: String? = null,
    val resumeFromSeconds: Double? = null,
    val durationMinutes: Double? = null,
    val audio: List<Track> = emptyList(),
    val subtitles: List<Track> = emptyList(),
    val segments: List<Segment> = emptyList(),
    /** The little pictures of the bar, when the film has been read for them. */
    val thumbnails: Thumbnails? = null,
    /** Where the film changes scene, when its file names them. */
    val chapters: List<Chapter> = emptyList(),
    /** What is done with the film's HDR, when it has any. */
    val wideGamut: WideGamutHandling? = null,
    /** Why the server plays it this way, as codes. */
    val reasons: List<JsonElement> = emptyList(),
    /** What is rebuilding the picture, when something is. */
    val rebuild: PictureRebuild? = null,
    /** What the file itself holds, beside what is made of it. */
    val film: FilmHolds? = null,
)

/** What rebuilds the picture: card or processor. */
@Serializable
data class PictureRebuild(val by: String, val codec: String, val height: Int? = null, val bitrate: Long? = null)

/** What the file holds, as the server's analyser read it. */
@Serializable
data class FilmHolds(
    val container: String? = null,
    val sizeBytes: Long = 0,
    val overallBitrate: Long? = null,
    val picture: PictureHeld? = null,
    /** The soundtrack being played, not the first of the file. */
    val sound: SoundHeld? = null,
)

@Serializable
data class PictureHeld(
    val codec: String,
    val profile: String? = null,
    /** The picture, margins off. */
    val width: Int,
    val height: Int,
    /** The frame it sits in, only when part of it is not the picture. */
    val frameWidth: Int? = null,
    val frameHeight: Int? = null,
    val frameRate: Double? = null,
    val bitrate: Long? = null,
    /** hdr10, hlg or dolby_vision. */
    val hdr: String? = null,
    val bitDepth: Int? = null,
)

@Serializable
data class SoundHeld(
    val codec: String,
    val channels: Int,
    val channelLayout: String? = null,
    val sampleRate: Int? = null,
    val bitrate: Long? = null,
)

/** The little pictures of a film, as sheets of many: a slash, the sheet's
 *  number and ".jpg" after [url] fetch one. */
@Serializable
data class Thumbnails(
    val url: String,
    /** How far apart in the film two of them stand. */
    val everySeconds: Double,
    /** One of them on its sheet, in pixels. */
    val width: Int,
    val height: Int,
    val columns: Int,
    val rows: Int,
    /** How many the film has: past the last, a sheet holds only black. */
    val counted: Int,
)

@Serializable
data class Chapter(val atSecond: Double, val title: String? = null)

@Serializable
data class WideGamutHandling(
    /** Converted to standard range, rather than handed over as it is. */
    val converted: Boolean,
    /** Whether the viewer's choice decided it: when it did not, choosing
     *  otherwise changes nothing. */
    val followsChoice: Boolean,
)

/** A film the server rebuilds as it is watched, and where to read it. */
@Serializable
data class StreamSession(val id: String, val playlistUrl: String)

/** How far the server has got making a rebuilt film: its step, starting,
 *  reading, producing or ready, and how much of the first piece a player
 *  needs is written, from nought to one. */
@Serializable
data class Preparation(
    val step: String = "starting",
    val firstWritten: Float = 0f,
    /** How hard the machine works at the film, while it does. */
    val producing: Producing? = null,
)

/** Pictures a second the server's tool writes, and that work against real
 *  time: below one, the picture will stop. */
@Serializable
data class Producing(@SerialName("pictures_a_second") val picturesASecond: Double = 0.0, val speed: Double = 0.0)

@Serializable
data class Heard(val stop: Boolean = false)

/** Whether an account gave itself a code for its televisions. */
@Serializable
data class TvCode(val set: Boolean = false)

/** What a library's grid can be narrowed by, and the letters its titles
 *  start with, each with how many works it holds. */
@Serializable
data class Filters(
    val genres: List<Counted> = emptyList(),
    val decades: List<CountedDecade> = emptyList(),
    /** Only the letters titles really start with, "#" first. */
    val initials: List<Counted> = emptyList(),
)

@Serializable
data class Counted(val name: String, val works: Int = 0)

@Serializable
data class CountedDecade(val decade: Int, val works: Int = 0)

/** Somebody of a cast or a crew, and what of theirs the library holds,
 *  newest first. */
@Serializable
data class Person(
    val id: String,
    val name: String,
    val biography: String? = null,
    /** Days written year, month and day. */
    val bornOn: String? = null,
    val diedOn: String? = null,
    val birthplace: String? = null,
    val photo: List<Picture> = emptyList(),
    val works: List<Card> = emptyList(),
)
