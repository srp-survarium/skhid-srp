package survarium.constants
{
   // A room request's answer (the server's RoomCode); PIN_NOT_4_DIGITS is the movie's own.
   public class RoomCode
   {
      
      public static const PIN_NOT_4_DIGITS:int = -1;
      
      public static const NONE:int = 0;
      
      public static const UNKNOWN:int = 2;
      
      public static const WRONG_PIN:int = 3;
      
      public static const FULL:int = 4;
      
      public static const IN_MATCH:int = 5;
      
      public static const TOO_MANY:int = 6;
      
      public static const OUTDATED:int = 7;
      
      public static const BAD:int = 8;
      
      public static const MAINTENANCE:int = 9;
      
      public static const KICKED:int = 10;
      
      public static const CLAN_TEAM:int = 11;
      
      public static const GEAR_TOO_HIGH:int = 12;
      
      public static const KIT_UNFIT:int = 13;
      
      // "Switch and enter": the preset could not be applied (presets.md).
      public static const PRESET_REFUSED:int = 14;
      
      // The kit a match would take can not play: Play and Join refused at
      // the door. Not shown: the buttons are greyed by the same rule
      // (presets.md, the owner's of 2026-10-23).
      public static const KIT_CANT_PLAY:int = 15;
      
      public function RoomCode()
      {
         super();
      }
   }
}
