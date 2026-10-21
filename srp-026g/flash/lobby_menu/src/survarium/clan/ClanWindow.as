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
      
      public static function isInClan(param1:String) : Boolean
      {
         return false;
      }
      
      public static function getMembersList() : Array
      {
         var _loc1_:Array;
         var _loc2_:uint;
         var _loc3_:Boolean;
         var _loc4_:uint;
         if(_clanMembers)
         {
            _loc1_ = [];
            _loc2_ = _clanMembers.length;
            _loc3_ = false;
            _loc4_ = 0;
            while(_loc4_ < _loc2_)
            {
               if(_loc3_ || _clanMembers[_loc4_].name != PlayerInfo.nickname)
               {
                  _loc1_.push(_clanMembers[_loc4_]);
               }
               else
               {
                  _loc3_ = true;
               }
               _loc4_++;
            }
            return _loc1_;
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
      
      public function setPlayerData(param1:Array) : void
      {
         if(this.createClanRenderer.visible)
         {
            this.createClanRenderer.setPlayerData(param1);
         }
         else
         {
            _clanMembers = param1;
            this.manageClanRenderer.setPlayerData(param1);
         }
      }
      
      public function setClanName(param1:String, param2:String) : void
      {
         this.manageClanRenderer.textFieldClan.text = param1 + " [" + param2 + "]";
         this.manageClanRenderer.textFieldLabel.text = LocalizationData.getName("clan") + " [" + param2 + "]";
         this.createClanRenderer.setClanName(param1,param2);
      }
      
      public function setState(param1:uint) : void
      {
         PlayerInfo.isClanManager = param1 == 1;
         if(this.lockBackground.visible)
         {
            this.lockBackground.visible = false;
            this.wheels.visible = false;
            this.wheels.playAnimation(false);
         }
         _state = param1;
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

