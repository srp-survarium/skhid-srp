package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import survarium.constants.Colors;
   import survarium.constants.CombatAction;
   
   public class CombatLogItem extends MovieClip
   {
      
      private var _data:Object;
      
      private var _states:Array = ["","kill","suicide","found","lost","bring"];
      
      private var _colorCode:Array = [Colors.OTHER_PLAYER,Colors.ENEMY,Colors.PLAYER];
      
      public var who_name:TextField;
      
      public var victim_name:TextField;
      
      public var object_icon:MovieClip;
      
      public var extra_icon:MovieClip;
      
      public var mastery_icon:MovieClip;

      private var spongeIcon:SpongeLogIcon;

      public function CombatLogItem()
      {
         super();
         this.who_name.autoSize = TextFieldAutoSize.LEFT;
         this.victim_name.autoSize = TextFieldAutoSize.LEFT;
      }
      
      public function setData(data:Object) : void
      {
         _data = data;
         var _action_id:uint = (data.action_id);
         var ll:* = this._states[_action_id];
         gotoAndStop(ll);
         if(this.who_name)
         {
            this.who_name.autoSize = TextFieldAutoSize.LEFT;
         }
         if(this.victim_name)
         {
            this.victim_name.autoSize = TextFieldAutoSize.LEFT;
         }
         this.who_name.text = data.who_name;
         if(this.object_icon)
         {
            this.object_icon.gotoAndStop(data.object_icon + 1);
            this.showSpongeIcon(data.object_icon == SpongeLogIcon.ICON);
         }
         if(_action_id == CombatAction.KILL)
         {
            this.victim_name.text = data.victim_name;
         }
         if(_action_id == CombatAction.KILL || _action_id == CombatAction.SUICIDE)
         {
            this.extra_icon.gotoAndStop(data.extra_icon + 1);
            this.mastery_icon.gotoAndStop(data.mastery_icon);
         }
         this.makeLayout();
      }
      
      public function makeLayout() : void
      {
         var x:uint;
         var dx:uint = 5;
         this.who_name.textColor = this._colorCode[this._data.who_team];
         x = this.who_name.x + this.who_name.width + dx;
         if(this.object_icon != null)
         {
            if(this.object_icon)
            {
               this.object_icon.x = x;
            }
            x += this.object_icon.width + dx;
         }
         if(this._data.extra_icon != 0 && this.extra_icon)
         {
            this.extra_icon.x = x;
            x += this.extra_icon.width + dx;
         }
         if(this._data.action_id == CombatAction.KILL)
         {
            this.victim_name.textColor = this._colorCode[this._data.victim_team];
            this.victim_name.x = x;
            x += this.victim_name.width + dx;
         }
         if(this._data.mastery_icon != 0 && this.mastery_icon)
         {
            this.mastery_icon.x = x;
            x += this.mastery_icon.width + dx;
         }
         this.x = 190 - width;
      }

      // The icon clip has no picture of the Sponge (its frame is empty): its picture is put in the
      // clip while its icon is shown, and taken out for any other.
      private function showSpongeIcon(shown:Boolean) : void
      {
         if(shown)
         {
            if(this.spongeIcon == null)
            {
               this.spongeIcon = new SpongeLogIcon();
            }
            if(this.spongeIcon.parent != this.object_icon)
            {
               this.object_icon.addChild(this.spongeIcon);
            }
         }
         else if(this.spongeIcon != null && this.spongeIcon.parent != null)
         {
            this.spongeIcon.parent.removeChild(this.spongeIcon);
         }
      }

      override public function toString() : String
      {
         return "[CombatLogLayout " + "]";
      }
   }
}

