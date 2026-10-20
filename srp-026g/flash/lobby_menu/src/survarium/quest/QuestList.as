package survarium.quest
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.interfaces.IDataRenderer;
   import survarium.data.LocalizationData;
   
   public class QuestList extends UIComponent implements IDataRenderer
   {
      
      public var list:ScrollingList;
      
      public var textFieldLabel:TextField;
      
      public var overlay:MovieClip;
      
      public var background:MovieClip;
      
      public var border:Button;
      
      public var arrow:MovieClip;
      
      private var _data:Array = [];
      
      private var _hidden:Boolean = false;
      
      private var _rollX:Number = 0;
      
      private var _animated:Boolean = false;
      
      public function QuestList()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.overlay.mouseEnabled = false;
         this.border.preventAutosizing = true;
         this.textFieldLabel.defaultTextFormat = this.textFieldLabel.getTextFormat();
         LocalizationData.addTextField(this.textFieldLabel,"quests");
         LocalizationData.addCallback(this.onLocal,"quests");
         this.arrow.mouseEnabled = false;
         this.border.addEventListener(ButtonEvent.CLICK,this.onRenderReaction);
         this.arrow.mouseChildren = false;
         this.arrow.visible = false;
         this.border.visible = false;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         super.enabled = value;
         alpha = enabled ? 1 : 0.7;
         mouseEnabled = enabled;
         mouseChildren = enabled;
      }
      
      private function onLocal(value:String = null) : void
      {
         this.border.tooltipRendererName = "SrvTextHintRenderer";
         this.updateBordrerHint();
      }
      
      private function updateBordrerHint() : void
      {
         this.border.data = {name:LocalizationData.getName(this._hidden ? "show_quest_list" : "hide_quest_list")};
      }
      
      private function onRollOut(e:Event) : void
      {
         if(!this._hidden || this._animated)
         {
            return;
         }
         _animated = true;
         TweenLite.to(this,0.2,{
            x:this._rollX,
            onComplete:this.onTweenEnd
         });
      }
      
      private function onRollOver(e:MouseEvent) : void
      {
         if(!this._hidden || this._animated)
         {
            return;
         }
         _animated = true;
         _rollX = x;
         TweenLite.to(this,0.2,{
            x:this._rollX - 30,
            onComplete:this.onTweenEnd
         });
      }
      
      private function onTweenEnd() : void
      {
         _animated = false;
      }
      
      protected function onRenderReaction(e:Event) : void
      {
         this.arrow.scaleX *= -1;
         _hidden = !this._hidden;
         this.updateBordrerHint();
         var shift:int = 75 * this.arrow.scaleX;
         layoutData.offsetH -= shift;
         _animated = true;
         TweenLite.killTweensOf(this);
         TweenLite.to(this,0.2,{x:x - shift});
         TweenLite.to(this.border,0.2,{
            x:(this._hidden ? -7 : -14),
            onComplete:this.onTweenEnd
         });
         TweenLite.to(this.arrow,0.2,{x:(this._hidden ? 1 : -6)});
         ExternalInterface.call("sound_play",17);
      }
      
      public function set data(value:Object) : void
      {
         _data = value as Array;
         invalidateData();
      }
      
      public function get data() : Object
      {
         return this._data;
      }
      
      override protected function draw() : void
      {
         var hh:int;
         trace("QuestList::",this.list,this._data);
         super.draw();
         if(isInvalid(InvalidationType.DATA) && this._data)
         {
            visible = this._data.length > 0;
            hh = Math.min(this._data.length,4) * this.list.rowHeight + 63;
            this.arrow.y = (this.border.height - this.arrow.height) / 2 + 8;
            this.list.height = Math.min(this._data.length,3) * this.list.rowHeight + this.list.margin * 2;
            this.list.dataProvider = new DataProvider(this._data);
         }
      }
   }
}

