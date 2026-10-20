package survarium.messages
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import survarium.Util;
   import survarium.controls.ItemInfoRenderer;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   
   public class MessageDailyReward extends MessageInfoBase
   {
      
      protected static var _hintRenderer:ItemInfoRenderer;
      
      public var btnClose:Button;
      
      public var rewardContainer:MovieClip;
      
      public var textFieldLabel:TextField;
      
      public var textFieldDescr:TextField;
      
      public var rewardsList:ButtonBar;
      
      protected const MESSAGE_WIDTH:uint = 604;
      
      private var delay:Number = 0.3;
      
      private var length:Number = 0.2;
      
      public function MessageDailyReward()
      {
         super();
         var _loc1_:Class = getDefinitionByName("InventoryItemHint") as Class;
         _hintRenderer = new _loc1_();
      }
      
      override public function get width() : Number
      {
         return this.MESSAGE_WIDTH;
      }
      
      override public function set width(param1:Number) : void
      {
         super.width = param1;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.btnClose.addEventListener(ButtonEvent.CLICK,onMessageAccept,false,0,true);
         var _loc1_:Array = _data.data as Array;
         var _loc2_:*;
         trace("FillLsitData",_loc1_,this.rewardsList.width,this.rewardsList.alpha,this.rewardsList.visible,_data.itemData);
         _hintRenderer.displaylabel(false);
         this.rewardsList.y -= 1;
         this.rewardsList.focusable = false;
         this.rewardsList.spacing = 0;
         this.rewardContainer.dropSlot.data = _data.itemData;
         this.rewardContainer.dropSlot.iconRenderer.width = 171;
         this.rewardContainer.dropSlot.iconRenderer.height = 83;
         this.rewardContainer.dropSlot.iconRenderer.x = -32;
         this.rewardContainer.dropSlot.iconRenderer.y = -10;
         if(_data.itemData)
         {
            addChild(_hintRenderer);
            _loc2_ = this.rewardContainer.dropSlot;
            this.rewardContainer.addEventListener(MouseEvent.MOUSE_OVER,this.showItemHint);
            this.rewardContainer.addEventListener(MouseEvent.MOUSE_OUT,this.hideItemHint);
            _hintRenderer.data = _data.itemData;
            _hintRenderer.visible = false;
            _hintRenderer.x = this.MESSAGE_WIDTH;
            _hintRenderer.y = this.rewardContainer.y;
         }
         this.rewardsList.dataProvider = new DataProvider(_loc1_);
         this.rewardsList.selectedIndex = _data.current_day - 1;
         this.textFieldLabel.styleSheet = MainScene.textStyleSheet;
         this.textFieldDescr.styleSheet = MainScene.textStyleSheet;
         this.rewardContainer.textFieldDescr.styleSheet = MainScene.textStyleSheet;
         trace("daily_bonus_label",LocalizationData.getName("daily_bonus_label"));
         this.textFieldLabel.text = LocalizationData.getName("daily_bonus_label") + ", " + Util.styledChar(PlayerInfo.nickname,"beige_bright") + "!";
         this.textFieldDescr.text = LocalizationData.getName("daily_bonus_description");
         this.rewardContainer.textFieldDescr.text = _data.completed ? LocalizationData.getName("daily_bonus_reward_complete") : LocalizationData.getName("daily_bonus_reward_description");
      }
      
      override protected function cleanUp(param1:Event = null) : void
      {
         super.cleanUp(param1);
         _hintRenderer.data = null;
         removeChild(_hintRenderer);
      }
      
      private function hideItemHint(param1:MouseEvent) : void
      {
         if(_hintRenderer.alpha != 0)
         {
            TweenLite.killTweensOf(_hintRenderer);
            TweenLite.to(_hintRenderer,this.length,{
               delay:this.delay,
               alpha:0
            });
         }
      }
      
      private function showItemHint(param1:MouseEvent) : void
      {
         _hintRenderer.visible = true;
         if(_hintRenderer.alpha == 0)
         {
            TweenLite.killTweensOf(_hintRenderer);
            TweenLite.to(_hintRenderer,this.length,{
               delay:this.delay,
               alpha:1
            });
         }
      }
   }
}

