package scaleform.clik.core
{
   import flash.display.DisplayObjectContainer;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.utils.Dictionary;
   import scaleform.clik.managers.FocusHandler;
   import scaleform.clik.managers.PopUpManager;
   import scaleform.gfx.Extensions;
   
   public dynamic class CLIK
   {
      
      public static var stage:Stage;
      
      public static var initialized:Boolean = false;
      
      public static var disableNullFocusMoves:Boolean = false;
      
      public static var disableDynamicTextFieldFocus:Boolean = false;
      
      public static var disableTextFieldToNullFocusMoves:Boolean = true;
      
      public static var useImmediateCallbacks:Boolean = false;
      
      protected static var isInitListenerActive:Boolean = false;
      
      protected static var firingInitCallbacks:Boolean = false;
      
      protected static var initQueue:Dictionary;
      
      protected static var validDictIndices:Vector.<uint>;
      
      public function CLIK()
      {
         super();
      }
      
      public static function initialize(stage:Stage, component:UIComponent) : void
      {
         if(initialized)
         {
            return;
         }
         CLIK.stage = stage;
         Extensions.enabled = true;
         initialized = true;
         FocusHandler.init(stage,component);
         PopUpManager.init(stage);
         initQueue = new Dictionary(true);
         validDictIndices = new Vector.<uint>();
      }
      
      public static function getTargetPathFor(clip:DisplayObjectContainer) : String
      {
         var targetPath:String;
         if(!clip.parent)
         {
            return clip.name;
         }
         targetPath = clip.name;
         return getTargetPathImpl(clip.parent as DisplayObjectContainer,targetPath);
      }
      
      public static function queueInitCallback(ref:UIComponent) : void
      {
         var path:String = getTargetPathFor(ref);
         var parents:Array;
         var numParents:uint;
         var dict:Dictionary;
         if(useImmediateCallbacks || firingInitCallbacks)
         {
            Extensions.CLIK_addedToStageCallback(ref.name,path,ref);
         }
         else
         {
            parents = path.split(".");
            numParents = parents.length - 1;
            dict = initQueue[numParents];
            if(dict == null)
            {
               dict = new Dictionary(true);
               initQueue[numParents] = dict;
               validDictIndices.push(numParents);
               if(validDictIndices.length > 1)
               {
                  validDictIndices.sort(sortFunc);
               }
            }
            dict[ref] = path;
            if(!isInitListenerActive)
            {
               isInitListenerActive = true;
               stage.addEventListener(Event.EXIT_FRAME,fireInitCallback,false,0,true);
            }
         }
      }
      
      protected static function fireInitCallback(event:Event) : void
      {
         var i:uint;
         var depth:uint;
         var depthQueue:Dictionary;
         var key:Object;
         var component:UIComponent;
         firingInitCallbacks = true;
         stage.removeEventListener(Event.EXIT_FRAME,fireInitCallback,false);
         isInitListenerActive = false;
         while(i < validDictIndices.length)
         {
            depth = validDictIndices[i];
            depthQueue = initQueue[depth] as Dictionary;
            for(key in depthQueue)
            {
               component = key as UIComponent;
               Extensions.CLIK_addedToStageCallback(component.name,depthQueue[component],component);
               depthQueue[component] = null;
            }
            i++;
         }
         validDictIndices.length = 0;
         clearQueue();
         firingInitCallbacks = false;
      }
      
      protected static function clearQueue() : void
      {
         var numDict:*;
         for(numDict in initQueue)
         {
            initQueue[numDict] = null;
         }
      }
      
      protected static function sortFunc(a:uint, b:uint) : Number
      {
         if(a < b)
         {
            return -1;
         }
         if(a > b)
         {
            return 1;
         }
         return 0;
      }
      
      protected static function getTargetPathImpl(clip:DisplayObjectContainer, targetPath:String = "") : String
      {
         var _name:String;
         if(!clip)
         {
            return targetPath;
         }
         _name = clip.name ? clip.name + "." : "";
         targetPath = _name + targetPath;
         return getTargetPathImpl(clip.parent as DisplayObjectContainer,targetPath);
      }
   }
}

