package survarium.friends
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.clik.controls.ToolTip;
   import scaleform.gfx.MouseEventEx;
   import survarium.constants.Colors;
   import survarium.controls.FixedTextDots;
   import survarium.controls.PlayerInfo;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   
   public class SquadListRender extends ListItemRenderer
   {
      
      public var playerName:FixedTextDots;
      
      public var readyText:TextField;
      
      public var iconReady:MovieClip;
      
      public var leadIcon:MovieClip;
      
      public var typeIcon:MovieClip;
      
      public var objectIcon:MovieClip;
      
      public var readyBack:MovieClip;
      
      public var tipArea:ToolTip;
      
      protected var statesEmpty:Vector.<String> = Vector.<String>(["empty_",""]);
      
      public function SquadListRender()
      {
         super();
      }
      
      override public function setData(param1:Object) : void
      {
         this.data = param1;
         setState(_state);
         invalidateData();
      }
      
      override protected function changeFocus() : void
      {
         super.changeFocus();
         setState("out");
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         addEventListener(MouseEvent.CLICK,this.onClick);
         displayFocus = false;
         focusable = false;
      }
      
      override protected function updateText() : void
      {
         trace("SLR::updateText",name,this.tipArea,this.playerName,this.iconReady,this.objectIcon,this.playerName,this.leadIcon,this.typeIcon);
         if(!_data)
         {
            return;
         }
         mouseChildren = true;
         this.leadIcon.visible = _data.index == 0;
         this.playerName.text = _data.name;
         this.playerName.textColor = _data.index == 0 ? Colors.PLAYER : 16777215;
         this.iconReady.visible = _data.ready && !this.leadIcon.visible;
         this.objectIcon.gotoAndStop(_data.weapon + 1);
         if(this.readyBack)
         {
            this.readyBack.visible = _data.ready;
         }
         this.readyText.text = _data.ready ? LocalizationData.getName("ready") : LocalizationData.getName("not_ready");
         if(_data.ready || this.leadIcon.visible)
         {
            this.readyText.x = this.leadIcon.visible ? 22 : 26;
         }
         else
         {
            this.readyText.x = (79 - this.readyText.textWidth) / 2;
         }
         this.readyText.textColor = _data.ready ? Colors.READY : Colors.NOT_READY;
         this.typeIcon.visible = _data.type != undefined;
         this.typeIcon.gotoAndStop(_data.type + 1);
      }
      
      override protected function getStatePrefixes() : Vector.<String>
      {
         if(!_data)
         {
            return this.statesEmpty;
         }
         return _selected ? statesSelected : statesDefault;
      }
      
      protected function sendMessage() : void
      {
         var _loc1_:String = data.name;
         ExternalInterface.call("start_friend_message",_loc1_);
         dispatchEvent(new Event(Event.CONNECT,true));
      }
      
      protected function removeFromSquad() : void
      {
         ExternalInterface.call("remove_from_squad",data.id);
      }
      
      protected function leaveSquad() : void
      {
         ExternalInterface.call("leave_squad");
      }
      
      protected function disbandSquad() : void
      {
         ExternalInterface.call("disband_squad");
      }
      
      protected function onClick(param1:MouseEventEx) : void
      {
         var _loc2_:Array;
         if(!data)
         {
            return;
         }
         if(param1.buttonIdx == 1)
         {
            _loc2_ = [];
            if(_data.name != PlayerInfo.nickname)
            {
               _loc2_.push({
                  name:LocalizationData.getName("send_message"),
                  click_function:this.sendMessage
               });
            }
            if(PlayerInfo.commander)
            {
               _loc2_.push({
                  name:LocalizationData.getName("disband_squad"),
                  click_function:this.disbandSquad
               });
               if(data.index != 0)
               {
                  _loc2_.push({
                     name:LocalizationData.getName("remove_from_squad"),
                     click_function:this.removeFromSquad
                  });
               }
               SrvContextMenu.show(_loc2_);
            }
            else
            {
               _loc2_.push({
                  name:LocalizationData.getName("leave_squad"),
                  click_function:this.leaveSquad
               });
               SrvContextMenu.show(_loc2_);
            }
         }
      }
   }
}

