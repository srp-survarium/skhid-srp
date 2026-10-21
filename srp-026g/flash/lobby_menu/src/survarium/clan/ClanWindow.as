package survarium.clan
{
   import flash.display.MovieClip;
   import flash.external.ExternalInterface;
   import survarium.controls.PlayerInfo;
   import survarium.controls.UIWindow;
   import survarium.data.LocalizationData;
   
   public class ClanWindow extends UIWindow
   {
      
      protected static var _state:uint = 0;
      
      private static var _clanMembers:Array;
      
      public var createClanRenderer:MovieClip;
      
      public var manageClanRenderer:MovieClip;
      
      public var wheels:MovieClip;
      
      public var lockBackground:MovieClip;
      
      public function ClanWindow()
      {
         super();
         this.manageClanRenderer.visible = false;
         this.createClanRenderer.visible = true;
         this.createClanRenderer.setState(_state);
      }
      
      public static function isInClan(playerName:String) : Boolean
      {
         return false;
      }
      
      public static function getMembersList() : Array
      {
         var members:Array;
         var count:uint;
         var selfSkipped:Boolean;
         var i:uint;
         if(_clanMembers)
         {
            members = [];
            count = _clanMembers.length;
            selfSkipped = false;
            i = 0;
            while(i < count)
            {
               if(selfSkipped || _clanMembers[i].name != PlayerInfo.nickname)
               {
                  members.push(_clanMembers[i]);
               }
               else
               {
                  selfSkipped = true;
               }
               i++;
            }
            return members;
         }
         return [];
      }
      
      public static function getState() : uint
      {
         return _state;
      }
      
      override public function get height() : Number
      {
         return 902;
      }
      
      override public function get width() : Number
      {
         return 580;
      }
      
      public function onActivate() : void
      {
         if(this.lockBackground.visible)
         {
            this.wheels.playAnimation(true);
         }
         trace("clanWindowOpened");
         ExternalInterface.call("clan_window_opened");
      }
      
      public function setPlayerData(players:Array) : void
      {
         if(this.createClanRenderer.visible)
         {
            this.createClanRenderer.setPlayerData(players);
         }
         else
         {
            _clanMembers = players;
            this.manageClanRenderer.setPlayerData(players);
         }
      }
      
      public function setClanName(clanName:String, abbreviation:String) : void
      {
         this.manageClanRenderer.textFieldClan.text = clanName + " [" + abbreviation + "]";
         this.manageClanRenderer.textFieldLabel.text = LocalizationData.getName("clan") + " [" + abbreviation + "]";
         this.createClanRenderer.setClanName(clanName,abbreviation);
      }
      
      public function setState(state:uint) : void
      {
         PlayerInfo.isClanManager = state == 1;
         if(this.lockBackground.visible)
         {
            this.lockBackground.visible = false;
            this.wheels.visible = false;
            this.wheels.playAnimation(false);
         }
         _state = state;
         if(_state < 3)
         {
            this.manageClanRenderer.visible = false;
            this.createClanRenderer.visible = true;
            this.createClanRenderer.setState(_state);
         }
         else
         {
            this.manageClanRenderer.visible = true;
            this.createClanRenderer.visible = false;
            this.manageClanRenderer.setState(_state);
         }
      }
   }
}

