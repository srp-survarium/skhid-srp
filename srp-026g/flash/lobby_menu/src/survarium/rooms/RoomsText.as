package survarium.rooms
{
    import survarium.data.LocalizationData;

    // The texts the rooms show by a number of the server's (the maps, modes
    // and weathers of the pool, a refused request's code, why a member cannot
    // go into the match), and the room's own chat lines: keys of srp's
    // dictionary texts (client-patches/localization).
    public class RoomsText
    {
        // By level (maps_dict); 10..13 are the ports of revision 23.
        private static const LEVELS:Array = [null, "st_srp_map_rudnya", "st_srp_map_school", "st_srp_map_vostok",
            "st_srp_map_chemical_plant", "st_srp_map_tarakanovsky_fort", "st_srp_map_mamayev_kurgan",
            "st_srp_map_vector", "st_srp_map_cologne", "st_srp_map_london", "st_srp_map_vostok_v021",
            "st_srp_map_mamayev_kurgan_v021", "st_srp_map_chemical_plant_v011", "st_srp_map_vostok_v011"];
        private static const MODES:Object = {
            "2": "st_srp_mode_protective_device",
            "3": "st_srp_mode_chargers",
            "4": "st_srp_mode_team_deathmatch",
            "5": "st_srp_mode_research",
            "6": "st_srp_mode_blackout"
        };
        // maps_dict's map_weather_id: 1 day, 2 rain, 3 evening, 4 night, 5 prerain.
        private static const WEATHER:Object = {
            "1": "st_srp_weather_day",
            "2": "st_srp_weather_rain",
            "3": "st_srp_weather_evening",
            "4": "st_srp_weather_night",
            "5": "st_srp_weather_before_rain"
        };
        // A refused request's code (RoomCode); RoomCode.NONE has none.
        private static const CODES:Object = {
            "2": "st_srp_rooms_code_2", "3": "st_srp_rooms_code_3", "4": "st_srp_rooms_code_4",
            "5": "st_srp_rooms_code_5", "6": "st_srp_rooms_code_6", "7": "st_srp_rooms_code_7",
            "8": "st_srp_rooms_code_8", "9": "st_srp_rooms_code_9", "10": "st_srp_rooms_code_10",
            "11": "st_srp_rooms_code_11", "12": "st_srp_rooms_code_12", "13": "st_srp_rooms_code_13",
            "14": "st_srp_rooms_code_14", "16": "st_srp_rooms_code_16", "17": "st_srp_rooms_code_17"
        };
        // Why a member waiting in its room cannot go into the match now
        // (`J;-1;<why>`, the owner's of 2026-10-20).
        private static const JOIN_WHY:Object = {
            "1": "st_srp_rooms_join_why_1", "2": "st_srp_rooms_join_why_2", "3": "st_srp_rooms_join_why_3",
            "4": "st_srp_rooms_join_why_4", "5": "st_srp_rooms_join_why_5", "6": "st_srp_rooms_join_why_6",
            "7": "st_srp_rooms_join_why_7"
        };
        // The room's own lines in its chat: the server says them in English
        // after the player's name.
        private static const NOTICES:Array = [
            [" joined the room", "st_srp_rooms_joined_room"],
            [" left the room", "st_srp_rooms_left_room"]
        ];

        // A level's name's key (the Matches window and the bot match menu
        // too), null for a level without one.
        public static function levelKey(level:int):String
        {
            return level >= 0 && level < LEVELS.length ? LEVELS[level] as String : null;
        }

        public static function levelName(level:int):String
        {
            var key:String = levelKey(level);
            return key != null ? LocalizationData.getName(key) : LocalizationData.getName("st_srp_rooms_level") + " " + level;
        }

        public static function weatherName(weather:int):String
        {
            var key:String = WEATHER[String(weather)] as String;
            return key != null ? LocalizationData.getName(key) : "";
        }

        public static function modeName(mode:int):String
        {
            var key:String = MODES[String(mode)] as String;
            return key != null ? LocalizationData.getName(key) : "";
        }

        // "" for a code without a text.
        public static function codeText(code:int):String
        {
            var key:String = CODES[String(code)] as String;
            return key != null ? LocalizationData.getName(key) : "";
        }

        // "" for no reason (or one without a text).
        public static function joinWhyText(why:int):String
        {
            var key:String = JOIN_WHY[String(why)] as String;
            return key != null ? LocalizationData.getName(key) : "";
        }

        // A room's own line in its chat in the game's language; any other
        // text as it is.
        public static function notice(text:String):String
        {
            for (var i:int = 0; i < NOTICES.length; i++)
            {
                var words:String = NOTICES[i][0];
                var at:int = text.length - words.length;
                if (at > 0 && text.substr(at) == words)
                    return LocalizationData.format(NOTICES[i][1], text.substr(0, at));
            }
            return text;
        }

        public static function clock(secs:int):String
        {
            if (secs < 0)
                secs = 0;
            var s:int = secs % 60;
            return int(secs / 60) + ":" + (s < 10 ? "0" : "") + s;
        }
    }
}
