package survarium.messages
{
   import fl.transitions.easing.Strong;
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.motion.Tween;
   import survarium.data.LocalizationData;
   
   public class MessageFactionUnlock extends MessageInfoBase
   {
      
      public var btnAccept:Button;
      
      public var textFieldLabel:TextField;
      
      public var textFieldSender:TextField;
      
      public var icon:MovieClip;
      
      public var iconLeft:MovieClip;
      
      public var iconRight:MovieClip;
      
      public var background:MovieClip;
      
      protected var _tweens:Array = [];
      
      private var _mWidth:uint = 581;
      
      private var _mHeight:uint = 414;
      
      public function MessageFactionUnlock()
      {
         super();
      }
      
      override public function get height() : Number
      {
         return this._mWidth;
      }
      
      override public function get width() : Number
      {
         return this._mHeight;
      }
      
      override protected function configUI() : void
      {
         this.textFieldLabel.defaultTextFormat = this.textFieldLabel.getTextFormat();
         this.textFieldSender.defaultTextFormat = this.textFieldSender.getTextFormat();
         this.textFieldLabel.text = _data.text;
         this.textFieldSender.text = _data.factionLabel;
         var ANIMATION_LENGTH:uint = 12500;
         this.icon.x = 231.85;
         this.icon.y = 110.75;
         this.icon.scaleX = 0.623291015625;
         this.icon.scaleY = 0.623291015625;
         this.icon.alpha = 0.140625;
         this.iconRight.x = 319.1;
         this.iconRight.y = -150.25;
         this.iconRight.scaleX = 2.5380859375;
         this.iconRight.scaleY = 2.5380859375;
         this.iconRight.alpha = 0.12109375;
         this.iconLeft.x = -105.2;
         this.iconLeft.y = -25.45;
         this.iconLeft.scaleX = 1.5803680419921875;
         this.iconLeft.scaleY = 1.5803680419921875;
         this.iconLeft.alpha = 0.1015625;
         var ease:Object = {ease:Strong.easeOut};
         this._tweens[0] = new Tween(ANIMATION_LENGTH,this.icon,{
            x:141.95,
            y:0.35,
            scaleX:1.4284820556640625,
            scaleY:1.4284820556640625,
            alpha:0.0703125
         },ease);
         this._tweens[1] = new Tween(ANIMATION_LENGTH,this.iconRight,{
            x:383,
            y:56.5,
            scaleX:1.0101318359375,
            scaleY:1.0101318359375,
            alpha:0.1796875
         },ease);
         this._tweens[2] = new Tween(ANIMATION_LENGTH,this.iconLeft,{
            x:39.4,
            y:110.55,
            scaleX:0.5766448974609375,
            scaleY:0.5766448974609375,
            alpha:0.12109375
         },ease);
         super.configUI();
         LocalizationData.addButton(this.btnAccept,"ok");
         this.iconLeft.gotoAndStop(_data.faction_id);
         this.iconRight.gotoAndStop(_data.faction_id);
         this.icon.gotoAndStop(_data.faction_id);
         this.background.gotoAndStop(_data.faction_id);
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.onMessageAccept,false,0,true);
         this.btnAccept.y -= 5;
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         super.onMessageAccept();
         for(var i:uint = 0; i < this._tweens.length; i++)
         {
            this._tweens[i] = null;
         }
      }
   }
}

