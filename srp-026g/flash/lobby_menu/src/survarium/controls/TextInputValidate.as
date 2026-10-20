package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import scaleform.clik.core.UIComponent;
   
   public class TextInputValidate extends UIComponent
   {
      
      public var textInput:SrvTextInput;
      
      public var loadingIndicator:MovieClip;
      
      public var warning:MovieClip;
      
      private var _lastText:String = null;
      
      private var _validateRequest:String = null;
      
      protected const MIN_CHARS:int = 3;
      
      protected const MAX_CHARS:int = 20;
      
      public function TextInputValidate()
      {
         super();
      }
      
      public function setRequest(param1:String) : void
      {
         _validateRequest = param1;
      }
      
      public function setText(param1:String) : void
      {
         this.textInput.removeEventListener(Event.CHANGE,this.updateText);
         this.textInput.text = param1;
         this.textInput.validateNow();
         this.textInput.addEventListener(Event.CHANGE,this.updateText);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.textInput.addEventListener(Event.CHANGE,this.updateText);
         this.textInput.maxChars = this.MAX_CHARS;
         this.warning.mouseEnabled = this.loadingIndicator.mouseEnabled = false;
         this.warning.visible = false;
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicator.visible = false;
         enabled = false;
      }
      
      private function requestTextValidation(param1:Event = null) : void
      {
         trace("requestTextValidation::",this.textInput.text);
         this.loadingIndicator.gotoAndPlay(1);
         this.loadingIndicator.visible = true;
         ExternalInterface.call(this._validateRequest,this.textInput.text);
      }
      
      private function updateText(param1:Event) : void
      {
         var _loc2_:String = this.textInput.text;
         trace("updateText::",_loc2_);
         _lastText = _loc2_;
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicator.visible = false;
         enabled = false;
         if(this.checkLocalTextValidation())
         {
            this.requestTextValidation();
         }
         else
         {
            dispatchEvent(new Event(Event.CHANGE));
         }
      }
      
      protected function checkLocalTextValidation() : Boolean
      {
         trace("checkLocalTextValidation",this._lastText);
         return this._lastText.length >= this.MIN_CHARS && this._lastText.length <= this.MAX_CHARS;
      }
      
      public function handleTextValidation(param1:Boolean, param2:String, param3:String = null) : void
      {
         trace("handleTextValidation",param1,param2,param3);
         if(this._lastText != param2)
         {
            return;
         }
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicator.visible = false;
         this.textInput.enabled = true;
         enabled = param1;
         if(param1 == false)
         {
            this.warning.showWarning(param3);
            this.warning.visible = true;
         }
         else
         {
            this.warning.visible = false;
         }
         dispatchEvent(new Event(Event.CHANGE));
      }
   }
}

